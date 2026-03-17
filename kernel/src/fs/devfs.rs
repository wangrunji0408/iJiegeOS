/// /dev 文件系统

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;
use super::{FileDescriptor, FileStat, DirEntry, Vfs};

pub struct DevFs;

impl Vfs for DevFs {
    fn open(&self, path: &str, flags: i32, _mode: u32) -> Option<Arc<dyn FileDescriptor>> {
        match path {
            "/null" => Some(Arc::new(DevNull)),
            "/zero" => Some(Arc::new(DevZero)),
            "/urandom" | "/random" => Some(Arc::new(DevRandom)),
            "/tty" | "/console" | "/ttyS0" => Some(Arc::new(DevTty)),
            _ => None,
        }
    }

    fn stat(&self, path: &str) -> Option<FileStat> {
        let mode = match path {
            "/null" | "/zero" | "/urandom" | "/random" | "/tty" | "/console" | "/ttyS0" => {
                0o020666  // char device
            }
            "/" => 0o040755,
            _ => return None,
        };
        Some(FileStat {
            st_mode: mode,
            st_nlink: 1,
            ..Default::default()
        })
    }

    fn readdir(&self, path: &str) -> Option<Vec<DirEntry>> {
        if path == "/" {
            Some(vec![
                DirEntry { name: String::from("null"), inode: 1, file_type: 2 },
                DirEntry { name: String::from("zero"), inode: 2, file_type: 2 },
                DirEntry { name: String::from("urandom"), inode: 3, file_type: 2 },
                DirEntry { name: String::from("random"), inode: 4, file_type: 2 },
                DirEntry { name: String::from("tty"), inode: 5, file_type: 2 },
            ])
        } else {
            None
        }
    }

    fn mkdir(&self, _: &str, _: u32) -> isize { -1 }
    fn unlink(&self, _: &str) -> isize { -1 }
    fn rmdir(&self, _: &str) -> isize { -1 }
    fn rename(&self, _: &str, _: &str) -> isize { -1 }
    fn link(&self, _: &str, _: &str) -> isize { -1 }
    fn symlink(&self, _: &str, _: &str) -> isize { -1 }
    fn readlink(&self, _: &str) -> Option<String> { None }
}

struct DevNull;
struct DevZero;
struct DevRandom;
struct DevTty;

impl FileDescriptor for DevNull {
    fn read(&self, _buf: &mut [u8]) -> isize { 0 }  // EOF
    fn write(&self, buf: &[u8]) -> isize { buf.len() as isize }
    fn stat(&self) -> FileStat {
        FileStat { st_mode: 0o020666, st_rdev: 0x0103, ..Default::default() }
    }
    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { true }
}

impl FileDescriptor for DevZero {
    fn read(&self, buf: &mut [u8]) -> isize {
        for b in buf.iter_mut() { *b = 0; }
        buf.len() as isize
    }
    fn write(&self, buf: &[u8]) -> isize { buf.len() as isize }
    fn stat(&self) -> FileStat {
        FileStat { st_mode: 0o020666, st_rdev: 0x0105, ..Default::default() }
    }
    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { true }
}

static RANDOM_SEED: Mutex<u64> = Mutex::new(0xdeadbeef12345678);

impl FileDescriptor for DevRandom {
    fn read(&self, buf: &mut [u8]) -> isize {
        let mut seed = RANDOM_SEED.lock();
        for b in buf.iter_mut() {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            *b = (*seed >> 33) as u8;
        }
        buf.len() as isize
    }
    fn write(&self, buf: &[u8]) -> isize { buf.len() as isize }
    fn stat(&self) -> FileStat {
        FileStat { st_mode: 0o020666, ..Default::default() }
    }
    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { true }
}

impl FileDescriptor for DevTty {
    fn read(&self, buf: &mut [u8]) -> isize { 0 }
    fn write(&self, buf: &[u8]) -> isize {
        for &c in buf {
            crate::arch::sbi::console_putchar(c);
        }
        buf.len() as isize
    }
    fn stat(&self) -> FileStat {
        FileStat { st_mode: 0o020666, ..Default::default() }
    }
    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { true }

    fn ioctl(&self, request: u64, arg: usize) -> isize {
        // TCGETS, TCSETS 等 tty ioctl 返回成功
        match request {
            0x5401 => 0,  // TCGETS
            0x5402 => 0,  // TCSETS
            0x5403 => 0,  // TCSETSW
            0x5404 => 0,  // TCSETSF
            0x5413 => {   // TIOCGWINSZ
                // 设置窗口大小为 80x24
                let ws = arg as *mut u16;
                unsafe {
                    *ws = 24;         // ws_row
                    *ws.add(1) = 80;  // ws_col
                    *ws.add(2) = 640; // ws_xpixel
                    *ws.add(3) = 480; // ws_ypixel
                }
                0
            }
            _ => 0,
        }
    }
}
