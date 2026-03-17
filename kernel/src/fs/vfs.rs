use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::string::String;

use super::FileStat;

/// 文件/目录项
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub inode: u64,
    pub file_type: u8,  // DT_REG=8, DT_DIR=4, DT_LNK=10
}

/// 文件打开选项
#[derive(Debug, Clone, Copy)]
pub struct OpenOptions {
    pub read: bool,
    pub write: bool,
    pub append: bool,
    pub create: bool,
    pub truncate: bool,
    pub exclusive: bool,
    pub nonblock: bool,
    pub cloexec: bool,
    pub directory: bool,
}

impl OpenOptions {
    pub fn from_flags(flags: i32) -> Self {
        let access = flags & 3;
        Self {
            read: access == 0 || access == 2,
            write: access == 1 || access == 2,
            append: flags & 0x400 != 0,
            create: flags & 0x40 != 0,
            truncate: flags & 0x200 != 0,
            exclusive: flags & 0x80 != 0,
            nonblock: flags & 0x800 != 0,
            cloexec: flags & 0x80000 != 0,
            directory: flags & 0x10000 != 0,
        }
    }
}

/// 文件描述符 trait
pub trait FileDescriptor: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> isize;
    fn write(&self, buf: &[u8]) -> isize;
    fn stat(&self) -> FileStat;
    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> isize {
        self.read(buf)
    }
    fn write_at(&self, offset: u64, buf: &[u8]) -> isize {
        self.write(buf)
    }
    fn seek(&self, offset: i64, whence: i32) -> i64 { -1 }
    fn get_offset(&self) -> u64 { 0 }
    fn truncate(&self, size: u64) -> isize { -1 }
    fn ioctl(&self, request: u64, arg: usize) -> isize { -25 }  // ENOTTY
    fn fcntl(&self, cmd: i32, arg: usize) -> isize { 0 }
    fn set_nonblock(&self, nonblock: bool) {}
    fn is_nonblock(&self) -> bool { false }
    fn can_read(&self) -> bool { self.is_readable() }
    fn can_write(&self) -> bool { self.is_writable() }
    fn has_error(&self) -> bool { false }
    fn is_directory(&self) -> bool { false }
    fn readdir(&self) -> Option<Vec<DirEntry>> { None }
    fn getdents(&self, buf: &mut [u8]) -> isize { -1 }
    fn get_path(&self) -> Option<String> { None }
    fn get_flags(&self) -> i32 { 0 }
    fn set_flags(&self, flags: i32) {}
    fn flock(&self, how: i32) -> isize { 0 }
    fn as_socket(&self) -> Option<&super::socket::Socket> { None }
}

/// 文件系统 trait
pub trait Vfs: Send + Sync {
    fn open(&self, path: &str, flags: i32, mode: u32) -> Option<Arc<dyn FileDescriptor>>;
    fn stat(&self, path: &str) -> Option<FileStat>;
    fn readdir(&self, path: &str) -> Option<Vec<DirEntry>>;
    fn mkdir(&self, path: &str, mode: u32) -> isize;
    fn unlink(&self, path: &str) -> isize;
    fn rmdir(&self, path: &str) -> isize;
    fn rename(&self, old: &str, new: &str) -> isize;
    fn link(&self, old: &str, new: &str) -> isize;
    fn symlink(&self, target: &str, link: &str) -> isize;
    fn readlink(&self, path: &str) -> Option<String>;
}

/// 通用文件 trait
pub trait VfsFile: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> isize;
    fn write(&self, buf: &[u8]) -> isize;
    fn seek(&self, offset: i64, whence: i32) -> i64;
    fn stat(&self) -> FileStat;
}

pub trait VfsNode: Send + Sync {
    fn get_type(&self) -> u8;
    fn stat(&self) -> FileStat;
    fn open(&self) -> Arc<dyn FileDescriptor>;
}
