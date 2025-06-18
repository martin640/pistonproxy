use std::net::{Shutdown, TcpListener};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::{spawn};
use std::time::SystemTime;
use env_logger::Env;
use log::{debug, info};
use crate::config::{get_config, BUFFER_SIZE, VERSION_PROTOCOL_NAME, VERSION_PROXY_NAME};
use crate::proxy::{ProxySocketInfo, ProxySocketState};

mod config;
mod packet;
mod proxy;
mod reader;
mod writer;
mod server_packets;
mod client_packets;
mod chat;

fn main() {
    let start_time = SystemTime::now();
    let config = get_config();
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    
    info!("pistonproxy version {}, protocol version {}", VERSION_PROXY_NAME, VERSION_PROTOCOL_NAME);
    
    let addr = format!("0.0.0.0:{}", config.settings.listen);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let connections = Arc::new(AtomicU32::new(0));
    
    info!("listening on {addr}");
    let startup_duration = start_time.elapsed().unwrap().as_micros();
    debug!("server is ready in {:.2} ms", (startup_duration as f32) / 1000.0);
    
    loop {
        let (stream, addr) = listener.accept().unwrap();
        debug!("[{}] accepted new connection", addr);
        if connections.load(Ordering::Relaxed) < config.settings.clients_limit {
            connections.fetch_add(1, Ordering::SeqCst);
            let connections_close = connections.clone();
            let addr_copy = addr.clone();
            spawn(move || {
                let stream_copy = stream.try_clone().unwrap();
                let socket_info_main: Arc<Mutex<ProxySocketInfo>> = Arc::new(Mutex::new(ProxySocketInfo {
                    state: ProxySocketState::Handshake,
                    last_activity: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
                    
                    client_addr: addr_copy,
                    client_socket: Some(stream_copy),
                    client_send_buffer: vec![0; BUFFER_SIZE],
                    client_send_buffer_len: 0,
                    
                    backend_addr: None,
                    backend_socket: None,
                    backend_send_buffer: vec![0; BUFFER_SIZE],
                    backend_send_buffer_len: 0,
                }));
                
                ProxySocketInfo::handle_client_connection(stream, addr_copy, socket_info_main);
                debug!("[{}] socket closed", addr);
                connections_close.fetch_sub(1, Ordering::SeqCst);
            });
        } else {
            debug!("clients_limit exceeded");
            stream.shutdown(Shutdown::Both).expect("stream close failed")
        }
    }
}
