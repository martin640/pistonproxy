use std::net::{Shutdown, TcpListener};
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::{spawn};
use std::time::SystemTime;
use env_logger::Env;
use log::{debug, info};
use crate::config::{get_config, BUFFER_SIZE, VERSION_PROTOCOL_NAME, VERSION_PROXY_NAME};
use crate::proxy::{ProxySocketInfo, ProxySocketState, SharedProxySocketInfo};
use crate::socket_tracker::SocketTracker;

mod config;
mod packet;
mod proxy;
mod reader;
mod writer;
mod server_packets;
mod client_packets;
mod chat;
mod socket_tracker;
mod utils;

fn main() {
    let start_time = SystemTime::now();
    let config = get_config();
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    
    info!("pistonproxy version {}, protocol version {}", VERSION_PROXY_NAME, VERSION_PROTOCOL_NAME);
    
    let addr = format!("0.0.0.0:{}", config.settings.listen);
    let listener = TcpListener::bind(addr.clone()).unwrap();
    
    let conn_counter = Arc::new(AtomicU32::new(0));
    let conn_tracker = SocketTracker::new();
    
    info!("listening on {addr}");
    let startup_duration = start_time.elapsed().unwrap().as_micros();
    debug!("server is ready in {:.2} ms", (startup_duration as f32) / 1000.0);
    
    loop {
        let (stream, addr) = listener.accept().unwrap();
        debug!("[{}] accepted new connection", addr);
        if conn_counter.load(Ordering::Relaxed) < config.settings.clients_limit {
            conn_counter.fetch_add(1, Ordering::SeqCst);
            
            let connections_close = conn_counter.clone();
            let addr_copy = addr.clone();
            
            let stream_copy = stream.try_clone().unwrap();
            let socket_info = SharedProxySocketInfo::new(addr_copy, stream_copy);
            let conn_id = conn_tracker.add_socket(&socket_info);
            let conn_tracker_copy = conn_tracker.clone();
            
            spawn(move || {
                socket_info.handle_connection();
                debug!("[{}] socket closed", addr);
                conn_tracker_copy.remove_socket(conn_id);
                connections_close.fetch_sub(1, Ordering::SeqCst);
            });
        } else {
            debug!("[{}] clients_limit exceeded", addr);
            stream.shutdown(Shutdown::Both).expect("failed to close stream")
        }
    }
}
