use alloc::vec::Vec;
use alloc::collections::VecDeque;

#[derive(Debug)]
pub struct SocketHandle {
    pub socket_type: SocketType,
    pub domain: i32,
    pub protocol: i32,
    pub nonblocking: bool,
    pub local_port: u16,
    pub local_addr: u32,
    pub remote_port: u16,
    pub remote_addr: u32,
    pub listening: bool,
    pub connected: bool,
    pub bound: bool,
    pub recv_buf: VecDeque<u8>,
    pub send_buf: Vec<u8>,
    pub backlog: i32,
    pub accept_queue: VecDeque<SocketHandle>,
    pub options: SocketOptions,
    pub closed: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum SocketType {
    Stream,
    Dgram,
    Raw,
}

#[derive(Debug, Clone, Default)]
pub struct SocketOptions {
    pub reuseaddr: bool,
    pub reuseport: bool,
    pub keepalive: bool,
    pub nodelay: bool,
}

impl SocketHandle {
    pub fn new(domain: i32, socktype: i32, protocol: i32) -> Self {
        let socket_type = match socktype & 0xf {
            1 => SocketType::Stream,
            2 => SocketType::Dgram,
            3 => SocketType::Raw,
            _ => SocketType::Stream,
        };
        let nonblocking = (socktype & 0x800) != 0;

        Self {
            socket_type,
            domain,
            protocol,
            nonblocking,
            local_port: 0,
            local_addr: 0,
            remote_port: 0,
            remote_addr: 0,
            listening: false,
            connected: false,
            bound: false,
            recv_buf: VecDeque::new(),
            send_buf: Vec::new(),
            backlog: 0,
            accept_queue: VecDeque::new(),
            options: SocketOptions::default(),
            closed: false,
        }
    }
}

/// Epoll instance
#[derive(Debug)]
pub struct EpollInstance {
    pub entries: Vec<EpollEntry>,
}

#[derive(Debug, Clone)]
pub struct EpollEntry {
    pub fd: i32,
    pub events: u32,
    pub data: u64,
}

impl EpollInstance {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, fd: i32, events: u32, data: u64) {
        self.entries.push(EpollEntry { fd, events, data });
    }

    pub fn modify(&mut self, fd: i32, events: u32, data: u64) {
        for entry in &mut self.entries {
            if entry.fd == fd {
                entry.events = events;
                entry.data = data;
                return;
            }
        }
    }

    pub fn delete(&mut self, fd: i32) {
        self.entries.retain(|e| e.fd != fd);
    }
}
