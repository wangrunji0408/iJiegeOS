use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;

#[derive(Debug)]
pub enum FileDescriptor {
    Stdin,
    Stdout,
    Stderr,
    /// A pipe endpoint
    Pipe {
        buffer: Vec<u8>,
        read_pos: usize,
    },
    /// A socket
    Socket {
        handle: Arc<Mutex<crate::net::SocketHandle>>,
    },
    /// An epoll instance
    Epoll {
        instance: Arc<Mutex<crate::net::EpollInstance>>,
    },
    /// eventfd
    EventFd {
        value: u64,
    },
    /// Regular file (backed by in-memory data)
    File {
        data: Vec<u8>,
        offset: usize,
        path: alloc::string::String,
    },
    /// /dev/null
    DevNull,
}

impl FileDescriptor {
    pub fn read(&mut self, buf: &mut [u8]) -> isize {
        match self {
            FileDescriptor::Stdin => {
                // Read from SBI console
                for b in buf.iter_mut() {
                    if let Some(c) = crate::arch::sbi::console_getchar() {
                        *b = c;
                    } else {
                        return 0;
                    }
                }
                buf.len() as isize
            }
            FileDescriptor::File { data, offset, .. } => {
                let remaining = data.len() - *offset;
                let len = core::cmp::min(buf.len(), remaining);
                buf[..len].copy_from_slice(&data[*offset..*offset + len]);
                *offset += len;
                len as isize
            }
            FileDescriptor::Pipe { buffer, read_pos } => {
                let remaining = buffer.len() - *read_pos;
                let len = core::cmp::min(buf.len(), remaining);
                buf[..len].copy_from_slice(&buffer[*read_pos..*read_pos + len]);
                *read_pos += len;
                len as isize
            }
            FileDescriptor::DevNull => 0,
            FileDescriptor::EventFd { value } => {
                if buf.len() >= 8 {
                    let bytes = value.to_le_bytes();
                    buf[..8].copy_from_slice(&bytes);
                    *value = 0;
                    8
                } else {
                    -22 // EINVAL
                }
            }
            FileDescriptor::Socket { .. } => {
                // Read from TCP socket via smoltcp
                loop {
                    crate::net::poll_net();
                    let ret = crate::net::tcp_read(buf);
                    if ret > 0 {
                        return ret;
                    }
                    // Spin wait briefly
                    for _ in 0..1000 { core::hint::spin_loop(); }
                }
            }
            _ => -9, // EBADF
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> isize {
        match self {
            FileDescriptor::Stdout | FileDescriptor::Stderr => {
                for &b in buf {
                    crate::arch::sbi::console_putchar(b);
                }
                buf.len() as isize
            }
            FileDescriptor::File { data, offset, .. } => {
                let end = *offset + buf.len();
                if end > data.len() {
                    data.resize(end, 0);
                }
                data[*offset..end].copy_from_slice(buf);
                *offset = end;
                buf.len() as isize
            }
            FileDescriptor::Pipe { buffer, .. } => {
                buffer.extend_from_slice(buf);
                buf.len() as isize
            }
            FileDescriptor::DevNull => buf.len() as isize,
            FileDescriptor::EventFd { value } => {
                if buf.len() >= 8 {
                    let v = u64::from_le_bytes(buf[..8].try_into().unwrap());
                    *value += v;
                    8
                } else {
                    -22 // EINVAL
                }
            }
            FileDescriptor::Socket { .. } => {
                // Write to TCP socket via smoltcp
                let mut sent = 0;
                while sent < buf.len() {
                    crate::net::poll_net();
                    let ret = crate::net::tcp_write(&buf[sent..]);
                    if ret > 0 {
                        sent += ret as usize;
                    } else {
                        for _ in 0..1000 { core::hint::spin_loop(); }
                    }
                }
                // Flush
                crate::net::poll_net();
                sent as isize
            }
            _ => -9, // EBADF
        }
    }
}
