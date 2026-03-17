use alloc::collections::VecDeque;
use alloc::sync::Arc;
use spin::Mutex;

use super::{FileDescriptor, FileStat};
use super::inode::InodeType;

const PIPE_BUF_SIZE: usize = 65536;

struct PipeInner {
    buf: VecDeque<u8>,
    writer_count: usize,
    reader_count: usize,
}

pub struct Pipe {
    inner: Arc<Mutex<PipeInner>>,
    is_read_end: bool,
}

impl Pipe {
    pub fn new_read(inner: Arc<Mutex<PipeInner>>) -> Self {
        Self { inner, is_read_end: true }
    }

    pub fn new_write(inner: Arc<Mutex<PipeInner>>) -> Self {
        Self { inner, is_read_end: false }
    }
}

pub fn create_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
    let inner = Arc::new(Mutex::new(PipeInner {
        buf: VecDeque::new(),
        writer_count: 1,
        reader_count: 1,
    }));
    (
        Arc::new(Pipe::new_read(inner.clone())),
        Arc::new(Pipe::new_write(inner)),
    )
}

impl FileDescriptor for Pipe {
    fn read(&self, buf: &mut [u8]) -> isize {
        if !self.is_read_end { return -1; }
        let mut inner = self.inner.lock();
        if inner.buf.is_empty() {
            if inner.writer_count == 0 {
                return 0;  // EOF
            }
            return -11;  // EAGAIN (非阻塞)
        }
        let n = buf.len().min(inner.buf.len());
        for i in 0..n {
            buf[i] = inner.buf.pop_front().unwrap();
        }
        n as isize
    }

    fn write(&self, buf: &[u8]) -> isize {
        if self.is_read_end { return -1; }
        let mut inner = self.inner.lock();
        if inner.reader_count == 0 {
            return -32;  // EPIPE
        }
        if inner.buf.len() + buf.len() > PIPE_BUF_SIZE {
            return -11;  // EAGAIN
        }
        for &b in buf {
            inner.buf.push_back(b);
        }
        buf.len() as isize
    }

    fn stat(&self) -> FileStat {
        FileStat {
            st_mode: InodeType::Pipe.mode_bits() | 0o644,
            ..Default::default()
        }
    }

    fn is_readable(&self) -> bool { self.is_read_end }
    fn is_writable(&self) -> bool { !self.is_read_end }

    fn can_read(&self) -> bool {
        let inner = self.inner.lock();
        !inner.buf.is_empty() || inner.writer_count == 0
    }

    fn can_write(&self) -> bool {
        let inner = self.inner.lock();
        inner.buf.len() < PIPE_BUF_SIZE && inner.reader_count > 0
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        let mut inner = self.inner.lock();
        if self.is_read_end {
            inner.reader_count -= 1;
        } else {
            inner.writer_count -= 1;
        }
    }
}
