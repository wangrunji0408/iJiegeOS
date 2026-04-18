//! Minimal epoll implementation on top of the File abstraction.
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Clone, Copy, Debug)]
pub struct EpollItem {
    pub fd: i32,
    pub events: u32,
    pub data: u64,
}

pub struct Epoll {
    pub items: Mutex<Vec<EpollItem>>,
}

impl Epoll {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { items: Mutex::new(Vec::new()) })
    }

    pub fn add(&self, ep: EpollItem) {
        self.items.lock().push(ep);
    }
    pub fn modify(&self, ep: EpollItem) {
        let mut items = self.items.lock();
        for it in items.iter_mut() {
            if it.fd == ep.fd { *it = ep; return; }
        }
        items.push(ep);
    }
    pub fn remove(&self, fd: i32) {
        self.items.lock().retain(|it| it.fd != fd);
    }
    pub fn snapshot(&self) -> Vec<EpollItem> {
        self.items.lock().clone()
    }
}

impl crate::fs::File for Epoll {
    fn read(&self, _buf: &mut [u8]) -> isize { -1 }
    fn write(&self, _buf: &[u8]) -> isize { -1 }
    fn pread(&self, _buf: &mut [u8], _off: u64) -> isize { -1 }
}
