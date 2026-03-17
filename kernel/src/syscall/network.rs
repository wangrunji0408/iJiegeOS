/// 网络系统调用

use alloc::sync::Arc;
use crate::fs::socket::Socket;
use crate::task::current_task;
use crate::mm::{translated_byte_buffer, translated_refmut};
use super::errno::*;

fn token() -> usize {
    crate::task::current_user_token()
}

pub fn sys_socket(domain: i32, sock_type: i32, protocol: i32) -> i64 {
    // AF_UNIX=1, AF_INET=2
    // SOCK_STREAM=1, SOCK_DGRAM=2, SOCK_NONBLOCK=0x800, SOCK_CLOEXEC=0x80000
    let actual_type = sock_type & !0x880000;  // 去掉 flags

    let socket = Arc::new(Socket::new(domain, actual_type, protocol));

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let fd = inner.alloc_fd();
    inner.fd_table[fd] = Some(socket);
    fd as i64
}

pub fn sys_bind(fd: usize, addr: *const u8, addrlen: u32) -> i64 {
    let tok = token();
    let addr_bytes = {
        let mut b = alloc::vec![0u8; addrlen as usize];
        let bufs = translated_byte_buffer(tok, addr, addrlen as usize);
        let mut off = 0;
        for buf in bufs {
            b[off..off + buf.len()].copy_from_slice(buf);
            off += buf.len();
        }
        b
    };

    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let socket = match file.as_socket() {
        Some(s) => s,
        None => return ENOTSOCK,
    };

    if let Some(sa) = crate::net::parse_sockaddr(&addr_bytes) {
        let mut inner = socket.inner.lock();
        inner.local_addr = Some(sa);
        inner.bound = true;
        0
    } else if addr_bytes.len() >= 2 {
        // AF_UNIX
        let mut inner = socket.inner.lock();
        inner.bound = true;
        0
    } else {
        EINVAL
    }
}

pub fn sys_listen(fd: usize, backlog: i32) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    // 设置监听状态
    if let Some(socket) = file.as_socket() {
        socket.inner.lock().listening = true;
    }
    0
}

pub fn sys_accept(fd: usize, addr: *mut u8, addrlen: *mut u32) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    let nonblock = file.is_nonblock();
    drop(inner);

    // 从 smoltcp 接受连接
    // 简化版本：使用内部缓冲
    let socket = match file.as_socket() {
        Some(s) => s,
        None => return ENOTSOCK,
    };

    // 尝试从网络接受连接
    crate::net::poll();

    // 如果没有待处理的连接
    if nonblock {
        return EAGAIN;
    }

    // 阻塞等待（让出CPU）
    loop {
        crate::net::poll();
        crate::task::suspend_current_and_run_next();

        let inner_guard = socket.inner.lock();
        if !inner_guard.recv_buf.is_empty() {
            break;
        }
        drop(inner_guard);
    }

    // 创建新 socket
    let new_socket = Arc::new(Socket::new(socket.inner.lock().domain, socket.inner.lock().sock_type, 0));
    new_socket.inner.lock().connected = true;

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(new_socket);
    new_fd as i64
}

pub fn sys_connect(fd: usize, addr: *const u8, addrlen: u32) -> i64 {
    let tok = token();
    let addr_bytes = {
        let mut b = alloc::vec![0u8; addrlen as usize];
        let bufs = translated_byte_buffer(tok, addr, addrlen as usize);
        let mut off = 0;
        for buf in bufs {
            b[off..off + buf.len()].copy_from_slice(buf);
            off += buf.len();
        }
        b
    };

    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    if let Some(sa) = crate::net::parse_sockaddr(&addr_bytes) {
        if let Some(socket) = file.as_socket() {
            let mut inner = socket.inner.lock();
            inner.peer_addr = Some(sa);
            inner.connected = true;
        }
        0
    } else {
        ECONNREFUSED
    }
}

pub fn sys_getsockname(fd: usize, addr: *mut u8, addrlen: *mut u32) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let tok = token();
    *translated_refmut(tok, addrlen) = 0;
    0
}

pub fn sys_getpeername(fd: usize, addr: *mut u8, addrlen: *mut u32) -> i64 {
    sys_getsockname(fd, addr, addrlen)
}

pub fn sys_sendto(fd: usize, buf: *const u8, len: usize, flags: i32, dest_addr: *const u8, addrlen: u32) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let tok = token();
    let bufs = translated_byte_buffer(tok, buf, len);
    let mut data = alloc::vec::Vec::new();
    for b in bufs {
        data.extend_from_slice(b);
    }
    file.write(&data) as i64
}

pub fn sys_recvfrom(fd: usize, buf: *mut u8, len: usize, flags: i32, src_addr: *mut u8, addrlen: *mut u32) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let mut kernel_buf = alloc::vec![0u8; len];
    let n = file.read(&mut kernel_buf);
    if n <= 0 { return n as i64; }

    let tok = token();
    let bufs = translated_byte_buffer(tok, buf, n as usize);
    let mut off = 0;
    for b in bufs {
        b.copy_from_slice(&kernel_buf[off..off + b.len()]);
        off += b.len();
    }

    if !addrlen.is_null() {
        *translated_refmut(tok, addrlen) = 0u32;
    }

    n as i64
}

pub fn sys_setsockopt(fd: usize, level: i32, optname: i32, optval: *const u8, optlen: u32) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    // 常见选项处理
    match (level, optname) {
        (1, 2) => {  // SOL_SOCKET, SO_REUSEADDR
            if let Some(socket) = file.as_socket() {
                socket.inner.lock().reuse_addr = true;
            }
        }
        (6, 1) => {  // IPPROTO_TCP, TCP_NODELAY
            if let Some(socket) = file.as_socket() {
                socket.inner.lock().tcp_nodelay = true;
            }
        }
        _ => {}
    }
    0
}

pub fn sys_getsockopt(fd: usize, level: i32, optname: i32, optval: *mut u8, optlen: *mut u32) -> i64 {
    let tok = token();
    if !optval.is_null() && !optlen.is_null() {
        *translated_refmut(tok, optval as *mut i32) = 0;
        *translated_refmut(tok, optlen) = 4u32;
    }
    0
}

pub fn sys_shutdown(fd: usize, how: i32) -> i64 {
    0
}

pub fn sys_sendmsg(fd: usize, msg: usize, flags: i32) -> i64 {
    // 简化：从 msghdr 结构中提取数据
    let tok = token();
    // struct msghdr: msg_name, msg_namelen, msg_iov, msg_iovlen, ...
    let msg_iov = *translated_refmut(tok, (msg + 16) as *mut usize);
    let msg_iovlen = *translated_refmut(tok, (msg + 24) as *mut usize);

    let mut total = 0i64;
    for i in 0..msg_iovlen {
        let iov_base = *translated_refmut(tok, (msg_iov + i * 16) as *mut usize);
        let iov_len = *translated_refmut(tok, (msg_iov + i * 16 + 8) as *mut usize);
        total += sys_sendto(fd, iov_base as *const u8, iov_len, flags, core::ptr::null(), 0);
    }
    total
}

pub fn sys_recvmsg(fd: usize, msg: usize, flags: i32) -> i64 {
    let tok = token();
    let msg_iov = *translated_refmut(tok, (msg + 16) as *mut usize);
    let msg_iovlen = *translated_refmut(tok, (msg + 24) as *mut usize);

    let mut total = 0i64;
    for i in 0..msg_iovlen {
        let iov_base = *translated_refmut(tok, (msg_iov + i * 16) as *mut usize);
        let iov_len = *translated_refmut(tok, (msg_iov + i * 16 + 8) as *mut usize);
        let n = sys_recvfrom(fd, iov_base as *mut u8, iov_len, flags, core::ptr::null_mut(), core::ptr::null_mut());
        if n < 0 { return n; }
        total += n;
        if (n as usize) < iov_len { break; }
    }
    total
}

pub fn sys_socketpair(domain: i32, sock_type: i32, protocol: i32, sv: *mut i32) -> i64 {
    // 创建管道对
    let (r, w) = crate::fs::create_pipe();

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let fd0 = inner.alloc_fd();
    inner.fd_table[fd0] = Some(r);
    let fd1 = inner.alloc_fd();
    inner.fd_table[fd1] = Some(w);
    drop(inner);

    let tok = token();
    *translated_refmut(tok, sv) = fd0 as i32;
    *translated_refmut(tok, unsafe { sv.add(1) }) = fd1 as i32;
    0
}

/// 为 dyn FileDescriptor 添加 downcast 支持
trait AsAnyRef {
    fn as_any_ref<T: 'static>(&self) -> Option<&T>;
}

impl<F: crate::fs::FileDescriptor + 'static> AsAnyRef for F {
    fn as_any_ref<T: 'static>(&self) -> Option<&T> {
        use core::any::Any;
        (self as &dyn Any).downcast_ref::<T>()
    }
}

// 为 dyn FileDescriptor 实现 as_any_ref
trait FileDescriptorExt {
    fn as_any_ref<T: 'static>(&self) -> Option<&T>;
}

impl FileDescriptorExt for dyn crate::fs::FileDescriptor {
    fn as_any_ref<T: 'static>(&self) -> Option<&T> {
        None  // 简化：不实现真正的 downcast
    }
}
