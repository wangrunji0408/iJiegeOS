use alloc::vec::Vec;

// Socket types for the file descriptor table
#[derive(Debug)]
pub struct SocketHandle {
    pub socket_type: SocketType,
    pub domain: i32,
    pub protocol: i32,
    pub nonblocking: bool,
    // For TCP
    pub local_addr: Option<SockAddr>,
    pub remote_addr: Option<SockAddr>,
    pub listening: bool,
    pub connected: bool,
    // Buffers
    pub recv_buf: Vec<u8>,
    pub send_buf: Vec<u8>,
    // Accept queue
    pub accept_queue: Vec<SocketHandle>,
}

#[derive(Debug, Clone, Copy)]
pub enum SocketType {
    Stream, // SOCK_STREAM (TCP)
    Dgram,  // SOCK_DGRAM (UDP)
    Raw,    // SOCK_RAW
}

#[derive(Debug, Clone)]
pub struct SockAddr {
    pub family: u16,
    pub port: u16,
    pub addr: [u8; 4], // IPv4
}

impl SocketHandle {
    pub fn new(domain: i32, socktype: i32, protocol: i32) -> Self {
        let socket_type = match socktype & 0xf {
            1 => SocketType::Stream,
            2 => SocketType::Dgram,
            3 => SocketType::Raw,
            _ => SocketType::Stream,
        };
        let nonblocking = (socktype & 0x800) != 0; // SOCK_NONBLOCK

        Self {
            socket_type,
            domain,
            protocol,
            nonblocking,
            local_addr: None,
            remote_addr: None,
            listening: false,
            connected: false,
            recv_buf: Vec::new(),
            send_buf: Vec::new(),
            accept_queue: Vec::new(),
        }
    }
}
