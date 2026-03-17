/// 套接字文件描述符（用于网络）
use alloc::sync::Arc;
use alloc::collections::VecDeque;
use spin::Mutex;
use super::{FileDescriptor, FileStat};

pub struct Socket {
    pub inner: Mutex<SocketInner>,
}

pub struct SocketInner {
    pub domain: i32,     // AF_INET=2, AF_UNIX=1
    pub sock_type: i32,  // SOCK_STREAM=1, SOCK_DGRAM=2
    pub protocol: i32,
    pub bound: bool,
    pub listening: bool,
    pub connected: bool,
    pub local_addr: Option<SockAddr>,
    pub peer_addr: Option<SockAddr>,
    pub recv_buf: VecDeque<u8>,
    pub send_buf: VecDeque<u8>,
    pub nonblock: bool,
    pub handle: Option<usize>,  // smoltcp socket handle
    pub reuse_addr: bool,
    pub tcp_nodelay: bool,
}

#[derive(Clone, Debug)]
pub struct SockAddr {
    pub port: u16,
    pub ip: [u8; 4],
}

impl Socket {
    pub fn new(domain: i32, sock_type: i32, protocol: i32) -> Self {
        Self {
            inner: Mutex::new(SocketInner {
                domain,
                sock_type,
                protocol,
                bound: false,
                listening: false,
                connected: false,
                local_addr: None,
                peer_addr: None,
                recv_buf: VecDeque::new(),
                send_buf: VecDeque::new(),
                nonblock: false,
                handle: None,
                reuse_addr: false,
                tcp_nodelay: false,
            }),
        }
    }
}

impl FileDescriptor for Socket {
    fn read(&self, buf: &mut [u8]) -> isize {
        crate::net::socket_recv(self, buf, 0)
    }

    fn write(&self, buf: &[u8]) -> isize {
        crate::net::socket_send(self, buf, 0)
    }

    fn stat(&self) -> FileStat {
        FileStat {
            st_mode: super::inode::InodeType::Socket.mode_bits() | 0o777,
            ..Default::default()
        }
    }

    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { true }

    fn can_read(&self) -> bool {
        let inner = self.inner.lock();
        !inner.recv_buf.is_empty() || inner.listening
    }

    fn can_write(&self) -> bool {
        self.inner.lock().connected
    }

    fn set_nonblock(&self, nonblock: bool) {
        self.inner.lock().nonblock = nonblock;
    }

    fn is_nonblock(&self) -> bool {
        self.inner.lock().nonblock
    }
}
