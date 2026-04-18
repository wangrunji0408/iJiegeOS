//! Filesystem abstraction and in-memory FS for the initial image.
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub trait File: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> isize;
    fn write(&self, buf: &[u8]) -> isize;
    fn seek(&self, _off: isize, _whence: u32) -> isize { -1 }
    fn size(&self) -> u64 { 0 }
    fn is_dir(&self) -> bool { false }
    fn readable(&self) -> bool { true }
    fn writable(&self) -> bool { true }
    /// Returns (offset in inode, serial). For socket/fifo: 0.
    fn inode_id(&self) -> u64 { 0 }
    fn get_dents(&self, _buf: &mut [u8]) -> isize { -1 }
    fn as_socket(&self) -> Option<&crate::net::Socket> { None }
}

/// Console streams.
pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl File for Stdin {
    fn read(&self, _buf: &mut [u8]) -> isize { 0 }
    fn write(&self, _buf: &[u8]) -> isize { -1 }
    fn writable(&self) -> bool { false }
}
impl File for Stdout {
    fn read(&self, _buf: &mut [u8]) -> isize { -1 }
    fn write(&self, buf: &[u8]) -> isize {
        for &b in buf { crate::sbi::console_putchar(b as usize); }
        buf.len() as isize
    }
    fn readable(&self) -> bool { false }
}
impl File for Stderr {
    fn read(&self, _buf: &mut [u8]) -> isize { -1 }
    fn write(&self, buf: &[u8]) -> isize {
        for &b in buf { crate::sbi::console_putchar(b as usize); }
        buf.len() as isize
    }
    fn readable(&self) -> bool { false }
}

// In-memory FS ----------------------------------------------------------

pub struct MemFile {
    pub path: String,
    pub data: Mutex<Vec<u8>>,
    pub pos: Mutex<u64>,
    pub writable: bool,
    pub readable: bool,
}

impl MemFile {
    pub fn new_rw(path: &str, data: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            path: path.into(),
            data: Mutex::new(data),
            pos: Mutex::new(0),
            writable: true,
            readable: true,
        })
    }
}

impl File for MemFile {
    fn read(&self, buf: &mut [u8]) -> isize {
        let d = self.data.lock();
        let mut p = self.pos.lock();
        let start = (*p as usize).min(d.len());
        let end = (start + buf.len()).min(d.len());
        let n = end - start;
        buf[..n].copy_from_slice(&d[start..end]);
        *p = end as u64;
        n as isize
    }
    fn write(&self, buf: &[u8]) -> isize {
        let mut d = self.data.lock();
        let mut p = self.pos.lock();
        let pos = *p as usize;
        if pos + buf.len() > d.len() { d.resize(pos + buf.len(), 0); }
        d[pos..pos + buf.len()].copy_from_slice(buf);
        *p = (pos + buf.len()) as u64;
        buf.len() as isize
    }
    fn seek(&self, off: isize, whence: u32) -> isize {
        let mut p = self.pos.lock();
        let d = self.data.lock();
        let new = match whence {
            0 => off as i64, // SEEK_SET
            1 => *p as i64 + off as i64, // SEEK_CUR
            2 => d.len() as i64 + off as i64, // SEEK_END
            _ => return -22,
        };
        if new < 0 { return -22; }
        *p = new as u64;
        new as isize
    }
    fn size(&self) -> u64 { self.data.lock().len() as u64 }
    fn readable(&self) -> bool { self.readable }
    fn writable(&self) -> bool { self.writable }
}

// -----------------------------------------------------------------------

pub struct MemFs {
    pub entries: Mutex<alloc::collections::BTreeMap<String, Vec<u8>>>,
}

impl MemFs {
    pub const fn new() -> Self { Self { entries: Mutex::new(alloc::collections::BTreeMap::new()) } }

    pub fn insert(&self, path: &str, data: Vec<u8>) {
        self.entries.lock().insert(path.into(), data);
    }

    pub fn open(&self, path: &str, _flags: u32) -> Option<Arc<dyn File>> {
        let e = self.entries.lock();
        let data = e.get(path)?.clone();
        Some(MemFile::new_rw(path, data))
    }

    pub fn exists(&self, path: &str) -> bool { self.entries.lock().contains_key(path) }
    pub fn size(&self, path: &str) -> Option<u64> { self.entries.lock().get(path).map(|v| v.len() as u64) }
}

lazy_static::lazy_static! {
    pub static ref ROOT_FS: MemFs = MemFs::new();
}

pub fn init() {
    // Initial files can be inserted by higher layers
    crate::println!("[kernel] memfs ready");
}
