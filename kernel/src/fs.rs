//! Filesystem abstraction and in-memory FS for the initial image.
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub trait File: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> isize;
    fn write(&self, buf: &[u8]) -> isize;
    fn seek(&self, _off: isize, _whence: u32) -> isize { -1 }
    fn pread(&self, buf: &mut [u8], off: u64) -> isize;
    fn size(&self) -> u64 { 0 }
    fn is_dir(&self) -> bool { false }
    fn readable(&self) -> bool { true }
    fn writable(&self) -> bool { true }
    fn inode_id(&self) -> u64 { 0 }
    fn path(&self) -> &str { "" }
    fn get_dents(&self, _buf: &mut [u8]) -> isize { -1 }
    fn as_socket(&self) -> Option<&SocketFile> { None }
    fn is_socket(&self) -> bool { false }
    /// For mmap: return raw bytes slice if backed by a memory file.
    fn as_bytes(&self) -> Option<&[u8]> { None }
}

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl File for Stdin {
    fn read(&self, _buf: &mut [u8]) -> isize { 0 }
    fn write(&self, _buf: &[u8]) -> isize { -1 }
    fn pread(&self, _buf: &mut [u8], _off: u64) -> isize { -1 }
    fn writable(&self) -> bool { false }
}
impl File for Stdout {
    fn read(&self, _buf: &mut [u8]) -> isize { -1 }
    fn write(&self, buf: &[u8]) -> isize {
        for &b in buf { crate::sbi::console_putchar(b as usize); }
        buf.len() as isize
    }
    fn pread(&self, _buf: &mut [u8], _off: u64) -> isize { -1 }
    fn readable(&self) -> bool { false }
}
impl File for Stderr {
    fn read(&self, _buf: &mut [u8]) -> isize { -1 }
    fn write(&self, buf: &[u8]) -> isize {
        for &b in buf { crate::sbi::console_putchar(b as usize); }
        buf.len() as isize
    }
    fn pread(&self, _buf: &mut [u8], _off: u64) -> isize { -1 }
    fn readable(&self) -> bool { false }
}

// Backed by a static byte slice — mmap-able without copying.
pub struct StaticFile {
    pub path: String,
    pub data: &'static [u8],
    pub pos: Mutex<u64>,
}

impl StaticFile {
    pub fn new(path: &str, data: &'static [u8]) -> Arc<Self> {
        Arc::new(Self { path: path.to_string(), data, pos: Mutex::new(0) })
    }
}

impl File for StaticFile {
    fn read(&self, buf: &mut [u8]) -> isize {
        let mut p = self.pos.lock();
        let start = (*p as usize).min(self.data.len());
        let end = (start + buf.len()).min(self.data.len());
        let n = end - start;
        buf[..n].copy_from_slice(&self.data[start..end]);
        *p = end as u64;
        n as isize
    }
    fn write(&self, _buf: &[u8]) -> isize { -1 }
    fn pread(&self, buf: &mut [u8], off: u64) -> isize {
        let start = (off as usize).min(self.data.len());
        let end = (start + buf.len()).min(self.data.len());
        let n = end - start;
        buf[..n].copy_from_slice(&self.data[start..end]);
        n as isize
    }
    fn seek(&self, off: isize, whence: u32) -> isize {
        let mut p = self.pos.lock();
        let new = match whence {
            0 => off as i64,
            1 => *p as i64 + off as i64,
            2 => self.data.len() as i64 + off as i64,
            _ => return -22,
        };
        if new < 0 { return -22; }
        *p = new as u64;
        new as isize
    }
    fn size(&self) -> u64 { self.data.len() as u64 }
    fn path(&self) -> &str { &self.path }
    fn as_bytes(&self) -> Option<&[u8]> { Some(self.data) }
    fn writable(&self) -> bool { false }
}

// RW in-memory file (log files etc.)
pub struct MemFile {
    pub path: String,
    pub data: Mutex<Vec<u8>>,
    pub pos: Mutex<u64>,
}

impl MemFile {
    pub fn new_rw(path: &str, data: Vec<u8>) -> Arc<Self> {
        Arc::new(Self { path: path.to_string(), data: Mutex::new(data), pos: Mutex::new(0) })
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
    fn pread(&self, buf: &mut [u8], off: u64) -> isize {
        let d = self.data.lock();
        let start = (off as usize).min(d.len());
        let end = (start + buf.len()).min(d.len());
        let n = end - start;
        buf[..n].copy_from_slice(&d[start..end]);
        n as isize
    }
    fn seek(&self, off: isize, whence: u32) -> isize {
        let mut p = self.pos.lock();
        let d = self.data.lock();
        let new = match whence {
            0 => off as i64,
            1 => *p as i64 + off as i64,
            2 => d.len() as i64 + off as i64,
            _ => return -22,
        };
        if new < 0 { return -22; }
        *p = new as u64;
        new as isize
    }
    fn size(&self) -> u64 { self.data.lock().len() as u64 }
    fn path(&self) -> &str { &self.path }
}

// --- Sockets (unchanged from previous impl) ----------------------------

pub enum SocketState {
    Unbound,
    Listening { port: u16 },
    Connected,
}

pub struct SocketFile {
    pub sock: Mutex<Option<crate::net::Socket>>,
    pub state: Mutex<SocketState>,
    pub nonblocking: core::sync::atomic::AtomicBool,
}

impl SocketFile {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sock: Mutex::new(crate::net::tcp_open()),
            state: Mutex::new(SocketState::Unbound),
            nonblocking: core::sync::atomic::AtomicBool::new(false),
        })
    }
}

impl File for SocketFile {
    fn read(&self, buf: &mut [u8]) -> isize {
        let guard = self.sock.lock();
        let Some(sock) = guard.as_ref() else { return -9; };
        if !self.nonblocking.load(core::sync::atomic::Ordering::Relaxed) {
            loop {
                crate::net::poll();
                if crate::net::tcp_can_recv(sock) { break; }
                if !crate::net::tcp_is_active(sock) { return 0; }
                unsafe { riscv::asm::wfi(); }
            }
        }
        crate::net::tcp_recv(sock, buf)
    }
    fn write(&self, buf: &[u8]) -> isize {
        let guard = self.sock.lock();
        let Some(sock) = guard.as_ref() else { return -9; };
        let mut total = 0isize;
        let mut off = 0usize;
        while off < buf.len() {
            loop {
                crate::net::poll();
                if crate::net::tcp_can_send(sock) { break; }
                if !crate::net::tcp_is_active(sock) { return if total == 0 { -1 } else { total }; }
                unsafe { riscv::asm::wfi(); }
            }
            let n = crate::net::tcp_send(sock, &buf[off..]);
            if n <= 0 { return if total == 0 { n } else { total }; }
            off += n as usize;
            total += n;
        }
        total
    }
    fn pread(&self, _buf: &mut [u8], _off: u64) -> isize { -29 } // ESPIPE
    fn readable(&self) -> bool { true }
    fn writable(&self) -> bool { true }
    fn as_socket(&self) -> Option<&SocketFile> { Some(self) }
    fn is_socket(&self) -> bool { true }
}

// --- Filesystem (path -> File) -----------------------------------------

pub struct VfsEntry {
    pub is_dir: bool,
    pub data: Option<&'static [u8]>,
    pub symlink: Option<String>,
}

pub struct Vfs {
    pub entries: Mutex<BTreeMap<String, VfsEntry>>,
}

impl Vfs {
    pub const fn new() -> Self { Self { entries: Mutex::new(BTreeMap::new()) } }

    pub fn insert_file(&self, path: &str, data: &'static [u8]) {
        self.entries.lock().insert(path.to_string(), VfsEntry { is_dir: false, data: Some(data), symlink: None });
    }
    pub fn insert_symlink(&self, path: &str, target: &str) {
        self.entries.lock().insert(path.to_string(), VfsEntry { is_dir: false, data: None, symlink: Some(target.to_string()) });
    }
    pub fn insert_dir(&self, path: &str) {
        self.entries.lock().insert(path.to_string(), VfsEntry { is_dir: true, data: None, symlink: None });
    }

    pub fn resolve(&self, path: &str) -> Option<String> {
        let mut cur = path.to_string();
        for _ in 0..8 {
            let e = self.entries.lock();
            let Some(ent) = e.get(&cur) else { return None; };
            if let Some(t) = &ent.symlink {
                cur = t.clone();
            } else {
                return Some(cur);
            }
        }
        None
    }

    pub fn open(&self, path: &str) -> Option<Arc<dyn File>> {
        let real = self.resolve(path)?;
        let e = self.entries.lock();
        let ent = e.get(&real)?;
        if ent.is_dir { return None; }
        let data = ent.data?;
        Some(StaticFile::new(&real, data))
    }

    pub fn exists(&self, path: &str) -> bool {
        let Some(real) = self.resolve(path) else { return false; };
        self.entries.lock().contains_key(&real)
    }

    pub fn size(&self, path: &str) -> Option<u64> {
        let real = self.resolve(path)?;
        let e = self.entries.lock();
        let ent = e.get(&real)?;
        ent.data.map(|d| d.len() as u64)
    }

    pub fn is_dir(&self, path: &str) -> Option<bool> {
        let real = self.resolve(path)?;
        let e = self.entries.lock();
        let ent = e.get(&real)?;
        Some(ent.is_dir)
    }
}

lazy_static::lazy_static! {
    pub static ref VFS: Vfs = Vfs::new();
}

pub fn init() {
    crate::initramfs::load(&VFS);
    let n = VFS.entries.lock().len();
    crate::println!("[kernel] vfs: {} entries", n);
}
