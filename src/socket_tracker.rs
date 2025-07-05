use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::AtomicUsize;
use crate::proxy::{ProxySocketInfo, SharedProxySocketInfo};

pub struct SocketTracker {
    id: Arc<AtomicUsize>,
    sockets: Arc<Mutex<Vec<(usize, Weak<Mutex<ProxySocketInfo>>)>>>
}

impl SocketTracker {
    pub fn new() -> SocketTracker {
        SocketTracker {
            id: Arc::new(AtomicUsize::new(0)),
            sockets: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn add_socket(&self, socket: &SharedProxySocketInfo) -> usize {
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut sockets = self.sockets.lock().unwrap();
        sockets.push((id, socket.weak()));
        id
    }
    
    pub fn remove_socket(&self, id: usize) {
        let mut sockets = self.sockets.lock().unwrap();
        sockets.retain(|(socket_id, _)| *socket_id != id);
    }
    
    pub fn size(&self) -> usize {
        self.sockets.lock().unwrap().len()
    }
}

impl Clone for SocketTracker {
    fn clone(&self) -> SocketTracker {
        SocketTracker {
            id: self.id.clone(),
            sockets: self.sockets.clone()
        }
    }
}