mod inode;
mod vfs;
mod tmpfs;
mod procfs;
mod devfs;
mod pipe;
pub mod socket;

pub use vfs::{Vfs, VfsNode, VfsFile, FileDescriptor, OpenOptions, DirEntry};
pub use inode::{Inode, InodeType};
pub use pipe::{Pipe, create_pipe};
pub use socket::Socket;

use alloc::sync::Arc;
use spin::Mutex;
use lazy_static::lazy_static;

/// 文件系统挂载点
pub struct MountPoint {
    pub path: alloc::string::String,
    pub fs: Arc<dyn Vfs>,
}

lazy_static! {
    static ref MOUNT_TABLE: Mutex<alloc::vec::Vec<MountPoint>> = Mutex::new(alloc::vec::Vec::new());
}

pub fn init() {
    // 挂载 tmpfs 作为根文件系统
    let rootfs = tmpfs::TmpFs::new();
    let rootfs = Arc::new(rootfs);

    // 创建基本目录结构
    rootfs.create_dir("/tmp");
    rootfs.create_dir("/var");
    rootfs.create_dir("/var/run");
    rootfs.create_dir("/var/log");
    rootfs.create_dir("/etc");
    rootfs.create_dir("/proc");
    rootfs.create_dir("/dev");
    rootfs.create_dir("/run");
    rootfs.create_dir("/usr");
    rootfs.create_dir("/usr/sbin");
    rootfs.create_dir("/usr/lib");
    rootfs.create_dir("/lib");

    // 加载 initrd 文件
    load_initrd(&rootfs);

    // 挂载
    MOUNT_TABLE.lock().push(MountPoint {
        path: alloc::string::String::from("/"),
        fs: rootfs,
    });

    log::info!("fs: root tmpfs mounted");
}

fn load_initrd(rootfs: &Arc<tmpfs::TmpFs>) {
    // initrd.cpio 在内核二进制中
    extern "C" {
        fn _initrd_start();
        fn _initrd_end();
    }

    let start = _initrd_start as usize;
    let end = _initrd_end as usize;
    let data = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };

    if data.is_empty() || end <= start {
        log::warn!("No initrd found");
        return;
    }

    log::info!("Loading initrd: {} bytes", data.len());
    cpio::extract_cpio(rootfs, data);
}

/// 根据路径打开文件
pub fn open(path: &str, flags: i32, mode: u32) -> Option<Arc<dyn FileDescriptor>> {
    let mount_table = MOUNT_TABLE.lock();
    for mount in mount_table.iter().rev() {
        if path.starts_with(mount.path.as_str()) {
            let rel_path = &path[mount.path.len()..];
            let rel_path = if rel_path.is_empty() { "/" } else { rel_path };
            return mount.fs.open(rel_path, flags, mode);
        }
    }
    None
}

/// 列出目录内容
pub fn readdir(path: &str) -> Option<alloc::vec::Vec<DirEntry>> {
    let mount_table = MOUNT_TABLE.lock();
    for mount in mount_table.iter().rev() {
        if path.starts_with(mount.path.as_str()) {
            let rel_path = &path[mount.path.len()..];
            let rel_path = if rel_path.is_empty() { "/" } else { rel_path };
            return mount.fs.readdir(rel_path);
        }
    }
    None
}

/// 获取文件信息
pub fn stat(path: &str) -> Option<FileStat> {
    let mount_table = MOUNT_TABLE.lock();
    for mount in mount_table.iter().rev() {
        if path.starts_with(mount.path.as_str()) {
            let rel_path = &path[mount.path.len()..];
            let rel_path = if rel_path.is_empty() { "/" } else { rel_path };
            return mount.fs.stat(rel_path);
        }
    }
    None
}

/// 读取符号链接目标
pub fn readlink(path: &str) -> Option<alloc::string::String> {
    let mount_table = MOUNT_TABLE.lock();
    for mount in mount_table.iter().rev() {
        if path.starts_with(mount.path.as_str()) {
            let rel_path = &path[mount.path.len()..];
            let rel_path = if rel_path.is_empty() { "/" } else { rel_path };
            return mount.fs.readlink(rel_path);
        }
    }
    None
}

/// Linux 兼容的文件统计信息
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FileStat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub __pad: u64,
    pub st_size: i64,
    pub st_blksize: i32,
    pub __pad2: i32,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: i64,
    pub st_mtime: i64,
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,
    pub st_ctime_nsec: i64,
    pub __unused: [u32; 2],
}

/// 标准输入/输出/错误的文件描述符实现
pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl FileDescriptor for Stdin {
    fn read(&self, buf: &mut [u8]) -> isize {
        // TODO: 实现键盘输入
        0
    }
    fn write(&self, buf: &[u8]) -> isize {
        -1  // stdin 不可写
    }
    fn stat(&self) -> FileStat {
        FileStat {
            st_mode: 0o20644,  // character device
            ..Default::default()
        }
    }
    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { false }
}

impl FileDescriptor for Stdout {
    fn read(&self, buf: &mut [u8]) -> isize { -1 }
    fn write(&self, buf: &[u8]) -> isize {
        for &c in buf {
            crate::arch::sbi::console_putchar(c);
        }
        buf.len() as isize
    }
    fn stat(&self) -> FileStat {
        FileStat {
            st_mode: 0o20644,
            ..Default::default()
        }
    }
    fn is_readable(&self) -> bool { false }
    fn is_writable(&self) -> bool { true }
}

impl FileDescriptor for Stderr {
    fn read(&self, buf: &mut [u8]) -> isize { -1 }
    fn write(&self, buf: &[u8]) -> isize {
        // 也写到串口
        for &c in buf {
            crate::arch::sbi::console_putchar(c);
        }
        buf.len() as isize
    }
    fn stat(&self) -> FileStat {
        FileStat {
            st_mode: 0o20644,
            ..Default::default()
        }
    }
    fn is_readable(&self) -> bool { false }
    fn is_writable(&self) -> bool { true }
}

pub mod cpio {
    use super::*;
    use alloc::string::String;

    /// CPIO newc 格式解析
    pub fn extract_cpio(rootfs: &Arc<tmpfs::TmpFs>, data: &[u8]) {
        let mut offset = 0;

        while offset + 110 <= data.len() {
            // CPIO newc 头部 (110 字节)
            let magic = &data[offset..offset+6];
            if magic != b"070701" && magic != b"070702" {
                break;
            }

            let namesize = parse_hex(&data[offset+94..offset+102]);
            let filesize = parse_hex(&data[offset+54..offset+62]);

            let name_start = offset + 110;
            let name_end = name_start + namesize;
            if name_end > data.len() { break; }

            let name = core::str::from_utf8(&data[name_start..name_end-1]).unwrap_or("unknown");

            if name == "TRAILER!!!" { break; }

            // 数据对齐到 4 字节
            let data_start = (name_end + 3) & !3;
            let data_end = data_start + filesize;
            if data_end > data.len() { break; }

            let mode = parse_hex(&data[offset+14..offset+22]);
            let file_data = &data[data_start..data_end];

            if !name.is_empty() && name != "." {
                let path = if name.starts_with("./") {
                    format!("/{}", &name[2..])
                } else if !name.starts_with('/') {
                    format!("/{}", name)
                } else {
                    String::from(name)
                };

                if mode & 0o170000 == 0o040000 {
                    // 目录
                    rootfs.create_dir(&path);
                } else if mode & 0o170000 == 0o100000 {
                    // 普通文件
                    rootfs.create_file(&path, file_data);
                } else if mode & 0o170000 == 0o120000 {
                    // 符号链接
                    if let Ok(target) = core::str::from_utf8(file_data) {
                        rootfs.create_symlink(&path, target);
                    }
                }
            }

            // 移动到下一个文件
            offset = (data_end + 3) & !3;
        }
    }

    fn parse_hex(s: &[u8]) -> usize {
        let s = core::str::from_utf8(s).unwrap_or("0");
        usize::from_str_radix(s, 16).unwrap_or(0)
    }
}

/// format! 宏需要的格式化支持（在 no_std 环境）
macro_rules! format {
    ($($arg:tt)*) => {{
        use alloc::string::ToString;
        alloc::format!($($arg)*)
    }};
}
use alloc::format;
