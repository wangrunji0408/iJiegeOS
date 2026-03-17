/// /proc 文件系统（虚拟文件系统）
/// 提供内核信息给用户空间

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::format;
use super::{FileDescriptor, FileStat, DirEntry, Vfs};

pub struct ProcFs;

impl Vfs for ProcFs {
    fn open(&self, path: &str, _flags: i32, _mode: u32) -> Option<Arc<dyn FileDescriptor>> {
        match path {
            "/self/maps" | "/self/smaps" => {
                Some(Arc::new(StaticFile::new(b"")))
            }
            "/self/status" => {
                let content = format!(
                    "Name:\tnginx\nPid:\t1\nPPid:\t0\nUid:\t0 0 0 0\nGid:\t0 0 0 0\n\
                     VmSize:\t1024 kB\nVmRSS:\t1024 kB\nThreads:\t1\n"
                );
                Some(Arc::new(StaticFile::new(content.as_bytes())))
            }
            "/self/stat" => {
                Some(Arc::new(StaticFile::new(b"1 (nginx) S 0 1 1 0 -1 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0\n")))
            }
            "/sys/kernel/ostype" => {
                Some(Arc::new(StaticFile::new(b"Linux\n")))
            }
            "/sys/kernel/osrelease" => {
                Some(Arc::new(StaticFile::new(b"5.15.0-iJiege\n")))
            }
            "/sys/kernel/hostname" => {
                Some(Arc::new(StaticFile::new(b"ijiege\n")))
            }
            "/meminfo" => {
                let content = b"MemTotal:       131072 kB\nMemFree:        65536 kB\n\
                    MemAvailable:   65536 kB\nBuffers:        1024 kB\nCached:         4096 kB\n";
                Some(Arc::new(StaticFile::new(content)))
            }
            "/cpuinfo" => {
                let content = b"processor\t: 0\nvendor_id\t: riscv64\n\
                    model name\t: iJiege RISC-V\nmhz\t\t: 10\n\n";
                Some(Arc::new(StaticFile::new(content)))
            }
            "/version" => {
                Some(Arc::new(StaticFile::new(
                    b"Linux version 5.15.0-iJiege (gcc version 11.0.0) #1 SMP\n"
                )))
            }
            "/net/if_inet6" => {
                Some(Arc::new(StaticFile::new(b"")))
            }
            "/net/fib_trie" => {
                Some(Arc::new(StaticFile::new(b"")))
            }
            _ => None,
        }
    }

    fn stat(&self, path: &str) -> Option<FileStat> {
        Some(FileStat {
            st_mode: 0o100444,  // regular file, read-only
            st_nlink: 1,
            ..Default::default()
        })
    }

    fn readdir(&self, path: &str) -> Option<Vec<DirEntry>> {
        match path {
            "/" => Some(vec![
                DirEntry { name: String::from("meminfo"), inode: 1, file_type: 8 },
                DirEntry { name: String::from("cpuinfo"), inode: 2, file_type: 8 },
                DirEntry { name: String::from("version"), inode: 3, file_type: 8 },
                DirEntry { name: String::from("self"), inode: 4, file_type: 4 },
            ]),
            _ => None,
        }
    }

    fn mkdir(&self, _path: &str, _mode: u32) -> isize { -1 }
    fn unlink(&self, _path: &str) -> isize { -1 }
    fn rmdir(&self, _path: &str) -> isize { -1 }
    fn rename(&self, _old: &str, _new: &str) -> isize { -1 }
    fn link(&self, _old: &str, _new: &str) -> isize { -1 }
    fn symlink(&self, _target: &str, _link: &str) -> isize { -1 }
    fn readlink(&self, _path: &str) -> Option<String> { None }
}

struct StaticFile {
    data: Vec<u8>,
    offset: spin::Mutex<usize>,
}

impl StaticFile {
    fn new(data: &[u8]) -> Self {
        Self { data: data.to_vec(), offset: spin::Mutex::new(0) }
    }
}

impl FileDescriptor for StaticFile {
    fn read(&self, buf: &mut [u8]) -> isize {
        let mut offset = self.offset.lock();
        let start = *offset;
        if start >= self.data.len() { return 0; }
        let end = (start + buf.len()).min(self.data.len());
        let n = end - start;
        buf[..n].copy_from_slice(&self.data[start..end]);
        *offset += n;
        n as isize
    }

    fn write(&self, _buf: &[u8]) -> isize { -1 }

    fn stat(&self) -> FileStat {
        FileStat {
            st_mode: 0o100444,
            st_size: self.data.len() as i64,
            ..Default::default()
        }
    }

    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { false }

    fn seek(&self, offset: i64, whence: i32) -> i64 {
        let mut cur = self.offset.lock();
        let new = match whence {
            0 => offset,
            1 => *cur as i64 + offset,
            2 => self.data.len() as i64 + offset,
            _ => return -1,
        };
        if new < 0 { return -1; }
        *cur = new as usize;
        new
    }
}
