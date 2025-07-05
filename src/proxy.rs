use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex, Weak};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration, SystemTime};
use log::{debug, warn};
use crate::chat::ChatData;
use crate::client_packets::{HandshakePacket, PingPacket};
use crate::config::{get_config, BUFFER_SIZE, VERSION_PROTOCOL_CODE, VERSION_PROTOCOL_NAME};
use crate::packet::{MinecraftPacket, MinecraftProtocolState, PacketParseError};
use crate::server_packets::{ServerPlayersInfo, ServerVersion, StatusPacket};
use crate::utils::bytes_as_hex;

#[derive(PartialEq)]
pub enum ProxySocketState {
    Handshake = 0,
    Closed = 1,
    Status = 2,
    Login = 4,
    Forward = 3,
}

impl Display for ProxySocketState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxySocketState::Handshake => write!(f, "Handshake"),
            ProxySocketState::Closed => write!(f, "Closed"),
            ProxySocketState::Status => write!(f, "Status"),
            ProxySocketState::Forward => write!(f, "Forward"),
            ProxySocketState::Login => write!(f, "Login"),
        }
    }
}

pub struct ProxySocketInfo {
    pub state: ProxySocketState,
    pub last_activity: u128,
    pub handshake_packet: Option<HandshakePacket>,
    pub disconnect_on_join: Option<String>,
    
    pub client_addr: SocketAddr,
    pub client_socket: Option<TcpStream>,
    pub client_send_buffer: Vec<u8>,
    pub client_send_buffer_len: usize,
    
    pub backend_addr: Option<SocketAddr>,
    pub backend_socket: Option<TcpStream>,
    pub backend_send_buffer: Vec<u8>,
    pub backend_send_buffer_len: usize,
}

pub struct SharedProxySocketInfo(Arc<Mutex<ProxySocketInfo>>);

impl SharedProxySocketInfo {
    pub fn new(addr: SocketAddr, socket: TcpStream) -> Self {
        Self(Arc::new(Mutex::new(ProxySocketInfo {
            state: ProxySocketState::Handshake,
            last_activity: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
            disconnect_on_join: None,
            handshake_packet: None,
            
            client_addr: addr,
            client_socket: Some(socket),
            client_send_buffer: vec![0; BUFFER_SIZE],
            client_send_buffer_len: 0,
            
            backend_addr: None,
            backend_socket: None,
            backend_send_buffer: vec![0; BUFFER_SIZE],
            backend_send_buffer_len: 0,
        })))
    }
    
    pub fn weak(&self) -> Weak<Mutex<ProxySocketInfo>> {
        Arc::downgrade(&self.0)
    }
    
    pub fn arc(&self) -> Arc<Mutex<ProxySocketInfo>> {
        self.0.clone()
    }
    
    pub fn handle_connection(&self) {
        let config = get_config();
        let buffer_size = config.settings.client_buffer_size;
        let mut buf: Vec<u8> = vec![0; buffer_size];
        let mut cursor = 0usize;
        let chunk = &mut [0u8; BUFFER_SIZE];
        let mut backend_thread_handle: Option<JoinHandle<_>> = None;
        
        let mut socket_info = self.0.lock().unwrap();
        let stream_owned = socket_info.client_socket.take().unwrap();
        let mut stream = stream_owned.try_clone().unwrap();
        socket_info.client_socket =  Some(stream_owned);
        let addr = socket_info.client_addr;
        drop(socket_info);
        
        while let Ok(len) = stream.read(chunk) {
            debug!("[{}] << received {} B chunk", addr, len);
            
            // lock is acquired only for time necessary to process the incoming chunk
            let mut socket_info = self.0.lock().unwrap();
            
            if len == 0 || socket_info.state == ProxySocketState::Closed {
                _ = stream.shutdown(Shutdown::Both);
                break
            }
            
            if (cursor + len) > buffer_size {
                warn!("[{}] client exceeded maximum input length ({} > {})", addr, cursor + len, buffer_size);
                socket_info.switch_state(ProxySocketState::Closed);
                _ = stream.shutdown(Shutdown::Both);
            } else {
                buf[cursor..(cursor + len)].copy_from_slice(&chunk[0..len]);
                cursor = cursor + len;
                debug!("[{}] :: buffer: {}", addr, bytes_as_hex(&buf[0..cursor]));
            }
            
            // try to parse packets in the buffer
            while cursor > 0 && socket_info.state != ProxySocketState::Forward && socket_info.state != ProxySocketState::Closed {
                let res = MinecraftPacket::parse_packet(buf[0..cursor].to_vec());
                if let Ok((packet, len)) = res {
                    debug!("[{}] accepted {} B packet", addr, len);
                    // shift buffer
                    buf.copy_within(len..cursor, 0);
                    cursor -= len;
                    
                    if socket_info.state == ProxySocketState::Handshake {
                        if packet.id == 0 { // handshake
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
                            socket_info.handshake_packet = Some(handshake_packet.clone());
                            
                            match handshake_packet.next_state {
                                MinecraftProtocolState::STATUS => socket_info.switch_state(ProxySocketState::Status),
                                MinecraftProtocolState::LOGIN => socket_info.switch_state(ProxySocketState::Login),
                                _ => {
                                    socket_info.switch_state(ProxySocketState::Closed);
                                    _ = stream.shutdown(Shutdown::Both);
                                }
                            }
                        } else if packet.id == 255 { // legacy 2-byte ping
                            debug!("received legacy ping, ignoring")
                        }
                    }
                    else if socket_info.state == ProxySocketState::Status {
                        if packet.id == 0 { // status request
                            if let Some(message) = socket_info.disconnect_on_join.take() {
                                let packet = MinecraftPacket::create_disconnect_packet(ChatData::new(message));
                                socket_info.write_packet(packet);
                                socket_info.switch_state(ProxySocketState::Closed);
                                _ = stream.shutdown(Shutdown::Both);
                            } else {
                                let packet = StatusPacket {
                                    version: ServerVersion {
                                        name: String::from(VERSION_PROTOCOL_NAME),
                                        protocol: VERSION_PROTOCOL_CODE
                                    },
                                    players: ServerPlayersInfo {
                                        max: 20,
                                        online: 0,
                                        sample: vec![],
                                    },
                                    description: ChatData::new_colored(String::from("Hello world"), String::from("#00ff00")),
                                    favicon: None,
                                    enforces_secure_chat: false,
                                };
                                let packet = MinecraftPacket::from(packet);
                                socket_info.write_packet(packet);
                            }
                        }
                        else if packet.id == 1 { // ping
                            let mut packet = packet;
                            let handshake_packet = PingPacket::try_from(&mut packet).unwrap();
                            
                            debug!("[{}] received ping timestamp={}", addr, handshake_packet.timestamp);
                            socket_info.write_packet(packet);
                        }
                    }
                    else if socket_info.state == ProxySocketState::Login {
                        let handshake_packet = socket_info.handshake_packet.as_ref().unwrap();
                        let endpoint = config.find_endpoint(handshake_packet.server_address.clone());
                        
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
                                    let addr_client = addr.clone();
                                    let addr: SocketAddr = origin.parse().unwrap();
                                    let self_clone = self.clone();
                                    match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
                                        Ok(stream) => {
                                            // write backend refs
                                            socket_info.backend_addr = Some(addr);
                                            socket_info.backend_socket =  Some(stream);
                                            backend_thread_handle = Some(spawn(move || {
                                                debug!("[{}] spawned backend worker", addr_client);
                                                self_clone.handle_backend_connection();
                                            }));
                                        }
                                        Err(e) => {
                                            warn!("[{}] unable to open socket to backend {}: {}", addr_client, addr, e);
                                            if socket_info.state == ProxySocketState::Forward {
                                                socket_info.switch_state(ProxySocketState::Status);
                                                socket_info.disconnect_on_join = Some("Bad Gateway".to_string());
                                            }
                                        }
                                    };
                                }
                            } else {
                                let default_message = String::from("Server configuration error");
                                let message = endpoint.message.as_ref().unwrap_or(&default_message);
                                let message = message.to_owned();
                                let packet = MinecraftPacket::create_disconnect_packet(ChatData::new_colored(message, String::from("#0ad4d9")));
                                socket_info.write_packet(packet);
                                socket_info.switch_state(ProxySocketState::Closed);
                                _ = stream.shutdown(Shutdown::Both);
                            }
                        } else {
                            let packet = MinecraftPacket::create_disconnect_packet(ChatData::new(String::from("Hello world!")));
                            socket_info.write_packet(packet);
                            socket_info.switch_state(ProxySocketState::Closed);
                            _ = stream.shutdown(Shutdown::Both);
                            // todo: send disconnect with default message
                        }
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
                        PacketParseError::PacketFormatError(msg) => {
                            debug!("[{}] failed to parse packet: PacketFormatError: {}", addr, msg);
                            break
                        },
                        PacketParseError::LengthMismatch => {
                            debug!("[{}] failed to parse packet: LengthMismatch", addr);
                            break
                        }
                    }
                }
            }
            
            if socket_info.state == ProxySocketState::Forward {
                let buffer_len = socket_info.backend_send_buffer_len;
                // push incoming buffer onto backend buffer and clear incoming buffer
                socket_info.backend_send_buffer[buffer_len..(buffer_len + cursor)].copy_from_slice(&buf[0..cursor]);
                socket_info.backend_send_buffer_len += cursor;
                cursor = 0;
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
    
    pub fn handle_backend_connection(&self) {
        let config = get_config();
        // acquire mutable socket info and set backend socket there
        let mut socket_info = self.0.lock().unwrap();
        let stream_owned = socket_info.backend_socket.take().unwrap();
        let mut stream = stream_owned.try_clone().unwrap();
        socket_info.backend_socket =  Some(stream_owned);
        let addr = socket_info.backend_addr.unwrap();
        drop(socket_info);
        
        let buffer_size = config.settings.backend_buffer_size;
        let mut buf: Vec<u8> = vec![0; buffer_size];
        let mut cursor = 0usize;
        let chunk = &mut [0u8; BUFFER_SIZE];
        
        while let Ok(len) = stream.read(chunk) {
            debug!("[{}] received {} B chunk", addr, len);
            
            let mut socket_info = self.0.lock().unwrap();
            
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

impl Clone for SharedProxySocketInfo {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl ProxySocketInfo {
    fn switch_state(&mut self, new_state: ProxySocketState) {
        debug!("[{}] switching state to {}", self.client_addr, new_state);
        self.state = new_state;
        self.last_activity = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
    }
    
    fn write_packet(&mut self, packet: MinecraftPacket) {
        let buf = packet.encode();
        debug!("[{}] >> sent {} B chunk", self.client_addr, buf.len());
        let stream_owned = self.client_socket.take().unwrap();
        let mut stream = stream_owned.try_clone().unwrap();
        self.client_socket =  Some(stream_owned);
        _ = stream.write(buf.as_slice());
    }
}
