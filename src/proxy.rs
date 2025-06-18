use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration, SystemTime};
use log::{debug, warn};
use crate::client_packets::HandshakePacket;
use crate::config::{get_config, BUFFER_SIZE};
use crate::packet::{MinecraftPacket, PacketParseError};

#[derive(PartialEq)]
pub enum ProxySocketState {
    Handshake = 0,
    Closed = 1,
    Status = 2,
    Forward = 3,
}

impl Display for ProxySocketState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxySocketState::Handshake => write!(f, "Handshake"),
            ProxySocketState::Closed => write!(f, "Closed"),
            ProxySocketState::Status => write!(f, "Status"),
            ProxySocketState::Forward => write!(f, "Forward"),
        }
    }
}

pub struct ProxySocketInfo {
    pub state: ProxySocketState,
    pub last_activity: u128,
    
    pub client_addr: SocketAddr,
    pub client_socket: Option<TcpStream>,
    pub client_send_buffer: Vec<u8>,
    pub client_send_buffer_len: usize,
    
    pub backend_addr: Option<SocketAddr>,
    pub backend_socket: Option<TcpStream>,
    pub backend_send_buffer: Vec<u8>,
    pub backend_send_buffer_len: usize,
}

impl ProxySocketInfo {
    fn switch_state(&mut self, new_state: ProxySocketState) {
        debug!("[{}] switching state to {}", self.client_addr, new_state);
        self.state = new_state;
        self.last_activity = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
    }
    
    pub fn handle_client_connection(mut stream: TcpStream, addr: SocketAddr, socket_info_main: Arc<Mutex<ProxySocketInfo>>) {
        let config = get_config();
        let buffer_size = config.settings.client_buffer_size;
        let mut buf: Vec<u8> = vec![0; buffer_size];
        let mut cursor = 0usize;
        let chunk = &mut [0u8; BUFFER_SIZE];
        let mut backend_thread_handle: Option<JoinHandle<_>> = None;
        
        while let Ok(len) = stream.read(chunk) {
            debug!("[{}] received {} B chunk", addr, len);
            
            // lock is acquired only for a time needed to process incoming chunk
            let mut socket_info = socket_info_main.lock().unwrap();
            
            if len == 0 || socket_info.state == ProxySocketState::Closed {
                _ = stream.shutdown(Shutdown::Both);
                break
            }
            
            if (cursor + len) > config.settings.client_buffer_size {
                warn!("[{}] client exceeded maximum input length ({} > {})", addr, cursor + len, config.settings.client_buffer_size);
                socket_info.switch_state(ProxySocketState::Closed);
                _ = stream.shutdown(Shutdown::Both);
            } else {
                buf[cursor..(cursor + len)].copy_from_slice(&chunk[0..len]);
                cursor = cursor + len;
            }
            
            if socket_info.state == ProxySocketState::Forward {
                let buffer_len = socket_info.backend_send_buffer_len;
                socket_info.backend_send_buffer[buffer_len..(buffer_len + cursor)].copy_from_slice(&buf[0..cursor]);
                cursor = 0;
            } else {
                // try to parse packets in the buffer
                while let res = MinecraftPacket::parse_packet(buf[0..cursor].to_vec()) {
                    if let Ok((packet, len)) = res {
                        debug!("[{}] accepted {} B packet", addr, len);
                        // shift buffer
                        buf.copy_within(len..cursor, 0);
                        cursor = cursor - len;
                        
                        // process packet
                        if packet.id == 0 { // classic ping
                            let mut packet = packet;
                            let handshake_packet = HandshakePacket::try_from(&mut packet).unwrap();
                            
                            debug!(
                                "[{}] received packet proto={}, addr={}, port={}, ns={:?}",
                                addr,
                                handshake_packet.protocol_version,
                                handshake_packet.server_address,
                                handshake_packet.server_port,
                                handshake_packet.next_state
                            );
                            
                            let endpoint = config.find_endpoint(handshake_packet.server_address);
                            if let Some(endpoint) = endpoint {
                                if let Some(origin) = &endpoint.origin {
                                    // switch state to forward so all data is forwarded to the proxy
                                    socket_info.switch_state(ProxySocketState::Forward);
                                    
                                    // before stopping the parsing loop, we should move incoming data to the backend send buffer
                                    let buffer_len = socket_info.backend_send_buffer_len;
                                    // todo: we shifted `buf` previously so the data should be pull from `packet` instead?
                                    socket_info.backend_send_buffer[buffer_len..(buffer_len + cursor)].copy_from_slice(&buf[0..cursor]);
                                    cursor = 0;
                                    
                                    // spawn backend worker thread
                                    if backend_thread_handle.is_none() {
                                        let socket_info_copy = Arc::clone(&socket_info_main);
                                        let addr_client = addr.clone();
                                        let addr: SocketAddr = origin.parse().unwrap();
                                        match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
                                            Ok(stream) => {
                                                backend_thread_handle = Some(spawn(move || {
                                                    debug!("[{}] spawned backend worker", addr_client);
                                                    ProxySocketInfo::handle_backend_connection(stream, addr, socket_info_copy);
                                                }));
                                            }
                                            Err(_) => {}
                                        };
                                    }
                                } else {
                                    let message = endpoint.message.clone();
                                    let message = message.unwrap_or("No further information".to_string());
                                    let packet = MinecraftPacket::create_disconnect_packet(&message);
                                    _ = stream.write(&packet.data);
                                    socket_info.switch_state(ProxySocketState::Closed);
                                    _ = stream.shutdown(Shutdown::Both);
                                }
                            } else {
                                let packet = MinecraftPacket::create_disconnect_packet(&"Hello world!".to_string());
                                _ = stream.write(&packet.data);
                                socket_info.switch_state(ProxySocketState::Closed);
                                _ = stream.shutdown(Shutdown::Both);
                                // todo: send disconnect with default message
                            }
                        } else if packet.id == 255 { // legacy 2-byte ping
                            debug!("received legacy ping, ignoring")
                        }
                    } else if let Err(e) = res {
                        match e {
                            PacketParseError::MalformedField(field) => {
                                debug!("[{}] failed to parse packet: MalformedField: {}", addr, field);
                                break
                            },
                            PacketParseError::EmptyBuffer => {
                                debug!("[{}] failed to parse packet: EmptyBuffer", addr);
                                break
                            },
                            PacketParseError::LengthMismatch => {
                                debug!("[{}] failed to parse packet: LengthMismatch", addr);
                                break
                            }
                        }
                    }
                }
            }
            
            if socket_info.backend_send_buffer_len > 0 {
                let buffer = socket_info.backend_send_buffer[0..socket_info.backend_send_buffer_len].to_vec();
                if let Some(backend_socket) = &mut socket_info.backend_socket {
                    _ = backend_socket.write(&buffer[..]);
                    socket_info.backend_send_buffer.clear();
                    socket_info.backend_send_buffer_len = 0;
                }
            }
        }
        
        if let Some(backend_thread) = backend_thread_handle {
            _ = backend_thread.join();
        }
    }
    
    pub fn handle_backend_connection(mut stream: TcpStream, addr: SocketAddr, socket_info_main: Arc<Mutex<ProxySocketInfo>>) {
        let config = get_config();
        // acquire mutable socket info and set backend socket there
        let mut socket_info = socket_info_main.lock().unwrap();
        let stream_copy = stream.try_clone().unwrap();
        socket_info.backend_addr = Some(addr);
        socket_info.backend_socket = Some(stream_copy);
        drop(socket_info);
        
        let buffer_size = config.settings.backend_buffer_size;
        let mut buf: Vec<u8> = vec![0; buffer_size];
        let mut cursor = 0usize;
        let chunk = &mut [0u8; BUFFER_SIZE];
        
        while let Ok(len) = stream.read(chunk) {
            debug!("[{}] received {} B chunk", addr, len);
            
            let mut socket_info = socket_info_main.lock().unwrap();
            
            if len == 0 || socket_info.state == ProxySocketState::Closed {
                _ = stream.shutdown(Shutdown::Both);
                break
            }
            
            if (cursor + len) > config.settings.backend_buffer_size {
                warn!("[{}] backend exceeded maximum input length ({} > {})", addr, cursor + len, config.settings.backend_buffer_size);
                socket_info.switch_state(ProxySocketState::Closed);
                _ = stream.shutdown(Shutdown::Both);
                if let Some(client_socket) = &socket_info.client_socket {
                    _ = client_socket.shutdown(Shutdown::Both);
                }
            } else {
                buf[cursor..(cursor + len)].copy_from_slice(&chunk[0..len]);
                cursor = cursor + len;
            }
            
            if socket_info.state == ProxySocketState::Forward {
                let mut send_buffer_len = socket_info.client_send_buffer_len;
                let send_buffer = socket_info.client_send_buffer[0..send_buffer_len].to_vec();
                if let Some(client_socket) = &mut socket_info.client_socket {
                    if send_buffer_len > 0 {
                        _ = client_socket.write(&send_buffer);
                        send_buffer_len = 0;
                    }
                    _ = client_socket.write(&buf[0..cursor]);
                    cursor = 0;
                }
                socket_info.client_send_buffer_len = send_buffer_len;
            } else {
                // TODO: save server status
            }
        }
    }
}
