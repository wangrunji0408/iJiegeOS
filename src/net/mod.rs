mod socket;

pub use socket::*;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    /// Global socket table mapping kernel socket FDs to socket handles
    pub static ref SOCKETS: Mutex<BTreeMap<usize, Arc<Mutex<SocketHandle>>>> = Mutex::new(BTreeMap::new());
    static ref NEXT_SOCKFD: Mutex<usize> = Mutex::new(100);
}

pub fn init() {
    println!("[NET] Socket layer initialized");
}

pub fn alloc_socket(sock: SocketHandle) -> usize {
    let mut next = NEXT_SOCKFD.lock();
    let fd = *next;
    *next += 1;
    SOCKETS.lock().insert(fd, Arc::new(Mutex::new(sock)));
    fd
}

pub fn get_socket(fd: usize) -> Option<Arc<Mutex<SocketHandle>>> {
    SOCKETS.lock().get(&fd).cloned()
}

pub fn remove_socket(fd: usize) {
    SOCKETS.lock().remove(&fd);
}
