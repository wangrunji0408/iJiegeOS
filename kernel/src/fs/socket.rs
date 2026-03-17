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
        // 已连接的 socket（有 smoltcp handle）：检查 smoltcp 是否有数据
        if let Some(raw_handle) = inner.handle {
            let handle: smoltcp::iface::SocketHandle = unsafe { core::mem::transmute(raw_handle) };
            drop(inner);
            return crate::net::tcp_can_recv(handle);
        }
        // 监听 socket：有待接受的连接
        if inner.listening {
            let port = inner.local_addr.as_ref().map(|a| a.port).unwrap_or(0);
            if port > 0 {
                return crate::net::tcp_has_pending(port);
            }
        }
        !inner.recv_buf.is_empty()
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

    fn as_socket(&self) -> Option<&Socket> { Some(self) }

    fn ioctl(&self, request: u64, arg: usize) -> isize {
        const FIONBIO: u64 = 0x5421;
        const FIONREAD: u64 = 0x541b;
        match request {
            FIONBIO => {
                // arg is a user-space pointer to int; translate via current page table
                let tok = crate::task::current_user_token();
                let val = *crate::mm::translated_ref(tok, arg as *const i32);
                self.inner.lock().nonblock = val != 0;
                0
            }
            FIONREAD => {
                let n = self.inner.lock().recv_buf.len();
                let tok = crate::task::current_user_token();
                *crate::mm::translated_refmut(tok, arg as *mut i32) = n as i32;
                0
            }
            _ => 0,
        }
    }
}
