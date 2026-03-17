/// 内存文件系统 (tmpfs)
/// 使用 BTreeMap 存储文件和目录

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use core::sync::atomic::{AtomicU64, Ordering};

use super::{FileDescriptor, FileStat, DirEntry, Vfs};
use super::inode::InodeType;

static NEXT_INO: AtomicU64 = AtomicU64::new(1);

fn alloc_ino() -> u64 {
    NEXT_INO.fetch_add(1, Ordering::SeqCst)
}

#[derive(Clone)]
pub enum FsNode {
    File(FileData),
    Dir(DirData),
    Symlink(String),
}

#[derive(Clone)]
pub struct FileData {
    pub ino: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub data: Vec<u8>,
    pub mtime: i64,
}

#[derive(Clone)]
pub struct DirData {
    pub ino: u64,
    pub mode: u32,
}

pub struct TmpFs {
    pub nodes: Mutex<BTreeMap<String, FsNode>>,
}

impl TmpFs {
    pub fn new() -> Self {
        let mut nodes = BTreeMap::new();
        // 根目录
        nodes.insert(String::from("/"), FsNode::Dir(DirData {
            ino: alloc_ino(),
            mode: 0o755,
        }));
        Self { nodes: Mutex::new(nodes) }
    }

    /// 创建目录
    pub fn create_dir(&self, path: &str) {
        let path = normalize_path(path);
        self.nodes.lock().entry(path).or_insert_with(|| {
            FsNode::Dir(DirData {
                ino: alloc_ino(),
                mode: 0o755,
            })
        });
    }

    /// 创建文件
    pub fn create_file(&self, path: &str, data: &[u8]) {
        let path = normalize_path(path);
        self.nodes.lock().insert(path, FsNode::File(FileData {
            ino: alloc_ino(),
            mode: 0o644,
            uid: 0,
            gid: 0,
            data: data.to_vec(),
            mtime: crate::timer::get_time_ms() as i64,
        }));
    }

    /// 创建符号链接
    pub fn create_symlink(&self, path: &str, target: &str) {
        let path = normalize_path(path);
        self.nodes.lock().insert(path, FsNode::Symlink(String::from(target)));
    }

    /// 获取节点（解析符号链接）
    fn get_node(&self, path: &str) -> Option<FsNode> {
        let path = normalize_path(path);
        let nodes = self.nodes.lock();
        let mut current = path.clone();
        let mut depth = 0;
        loop {
            match nodes.get(&current) {
                Some(FsNode::Symlink(target)) => {
                    depth += 1;
                    if depth > 40 { return None; }
                    current = if target.starts_with('/') {
                        normalize_path(target)
                    } else {
                        let dir = parent_dir(&current);
                        normalize_path(&alloc::format!("{}/{}", dir, target))
                    };
                }
                Some(node) => return Some(node.clone()),
                None => return None,
            }
        }
    }
}

impl Vfs for TmpFs {
    fn open(&self, path: &str, flags: i32, mode: u32) -> Option<Arc<dyn FileDescriptor>> {
        let opts = super::vfs::OpenOptions::from_flags(flags);
        let path_str = normalize_path(path);

        let node = {
            let mut nodes = self.nodes.lock();
            if opts.create {
                nodes.entry(path_str.clone()).or_insert_with(|| {
                    FsNode::File(FileData {
                        ino: alloc_ino(),
                        mode: mode & 0o777,
                        uid: 0, gid: 0,
                        data: Vec::new(),
                        mtime: crate::timer::get_time_ms() as i64,
                    })
                }).clone()
            } else {
                nodes.get(&path_str)?.clone()
            }
        };

        match node {
            FsNode::File(mut file_data) => {
                if opts.truncate {
                    file_data.data.clear();
                    self.nodes.lock().insert(path_str.clone(), FsNode::File(file_data.clone()));
                }
                Some(Arc::new(TmpFile {
                    path: path_str,
                    fs: Arc::clone(&Arc::new(self as *const TmpFs as usize)),
                    ino: file_data.ino,
                    data: Mutex::new(file_data.data),
                    offset: Mutex::new(0),
                    writable: opts.write,
                    readable: opts.read,
                    append: opts.append,
                    nonblock: Mutex::new(opts.nonblock),
                    flags: Mutex::new(flags),
                }))
            }
            FsNode::Dir(_) => {
                Some(Arc::new(TmpDir {
                    path: path_str,
                    fs_ptr: Arc::new(self as *const TmpFs as usize),
                    offset: Mutex::new(0),
                }))
            }
            FsNode::Symlink(target) => {
                // 符号链接：打开目标
                let resolved = if target.starts_with('/') {
                    target
                } else {
                    let dir = parent_dir(path);
                    alloc::format!("{}/{}", dir, target)
                };
                self.open(&resolved, flags, mode)
            }
        }
    }

    fn stat(&self, path: &str) -> Option<FileStat> {
        let path = normalize_path(path);
        let nodes = self.nodes.lock();
        let node = nodes.get(&path)?;
        let now = crate::timer::get_time_ms() as i64 / 1000;
        match node {
            FsNode::File(f) => Some(FileStat {
                st_ino: f.ino,
                st_mode: InodeType::Regular.mode_bits() | (f.mode & 0o777),
                st_nlink: 1,
                st_uid: f.uid,
                st_gid: f.gid,
                st_size: f.data.len() as i64,
                st_blksize: 4096,
                st_blocks: ((f.data.len() + 511) / 512) as i64,
                st_atime: now,
                st_mtime: f.mtime / 1000,
                st_ctime: f.mtime / 1000,
                ..Default::default()
            }),
            FsNode::Dir(d) => Some(FileStat {
                st_ino: d.ino,
                st_mode: InodeType::Directory.mode_bits() | (d.mode & 0o777),
                st_nlink: 2,
                st_blksize: 4096,
                st_atime: now,
                st_mtime: now,
                st_ctime: now,
                ..Default::default()
            }),
            FsNode::Symlink(target) => Some(FileStat {
                st_mode: InodeType::Symlink.mode_bits() | 0o777,
                st_size: target.len() as i64,
                st_atime: now,
                st_mtime: now,
                st_ctime: now,
                ..Default::default()
            }),
        }
    }

    fn readdir(&self, path: &str) -> Option<Vec<DirEntry>> {
        let path = normalize_path(path);
        let nodes = self.nodes.lock();

        // 检查是否是目录
        match nodes.get(&path)? {
            FsNode::Dir(_) => {}
            _ => return None,
        }

        let prefix = if path == "/" {
            String::from("/")
        } else {
            alloc::format!("{}/", path)
        };

        let mut entries = Vec::new();
        entries.push(DirEntry { name: String::from("."), inode: 0, file_type: 4 });
        entries.push(DirEntry { name: String::from(".."), inode: 0, file_type: 4 });

        for (node_path, node) in nodes.iter() {
            if node_path == &path { continue; }
            if !node_path.starts_with(&prefix.as_str()) { continue; }
            let rest = &node_path[prefix.len()..];
            // 只取直接子项（不含 /）
            if rest.contains('/') { continue; }

            let file_type = match node {
                FsNode::File(_) => 8,
                FsNode::Dir(_) => 4,
                FsNode::Symlink(_) => 10,
            };
            let ino = match node {
                FsNode::File(f) => f.ino,
                FsNode::Dir(d) => d.ino,
                FsNode::Symlink(_) => 0,
            };
            entries.push(DirEntry {
                name: String::from(rest),
                inode: ino,
                file_type,
            });
        }

        Some(entries)
    }

    fn mkdir(&self, path: &str, mode: u32) -> isize {
        self.create_dir(path);
        0
    }

    fn unlink(&self, path: &str) -> isize {
        let path = normalize_path(path);
        let mut nodes = self.nodes.lock();
        if nodes.remove(&path).is_some() { 0 } else { -1 }
    }

    fn rmdir(&self, path: &str) -> isize {
        self.unlink(path)
    }

    fn rename(&self, old: &str, new: &str) -> isize {
        let old = normalize_path(old);
        let new = normalize_path(new);
        let mut nodes = self.nodes.lock();
        if let Some(node) = nodes.remove(&old) {
            nodes.insert(new, node);
            0
        } else {
            -1
        }
    }

    fn link(&self, old: &str, new: &str) -> isize {
        let old = normalize_path(old);
        let new = normalize_path(new);
        let mut nodes = self.nodes.lock();
        if let Some(node) = nodes.get(&old).cloned() {
            nodes.insert(new, node);
            0
        } else {
            -1
        }
    }

    fn symlink(&self, target: &str, link: &str) -> isize {
        let link = normalize_path(link);
        self.nodes.lock().insert(link, FsNode::Symlink(String::from(target)));
        0
    }

    fn readlink(&self, path: &str) -> Option<String> {
        let path = normalize_path(path);
        let nodes = self.nodes.lock();
        match nodes.get(&path)? {
            FsNode::Symlink(target) => Some(target.clone()),
            _ => None,
        }
    }
}

/// TmpFs 文件描述符
pub struct TmpFile {
    path: String,
    fs: Arc<usize>,  // *const TmpFs as usize
    ino: u64,
    data: Mutex<Vec<u8>>,
    offset: Mutex<u64>,
    writable: bool,
    readable: bool,
    append: bool,
    nonblock: Mutex<bool>,
    flags: Mutex<i32>,
}

impl TmpFile {
    fn get_fs(&self) -> &TmpFs {
        unsafe { &*(*self.fs as *const TmpFs) }
    }
}

unsafe impl Send for TmpFile {}
unsafe impl Sync for TmpFile {}

impl FileDescriptor for TmpFile {
    fn read(&self, buf: &mut [u8]) -> isize {
        if !self.readable { return -1; }
        let data = self.data.lock();
        let mut offset = self.offset.lock();
        let start = *offset as usize;
        if start >= data.len() { return 0; }
        let end = (start + buf.len()).min(data.len());
        let n = end - start;
        buf[..n].copy_from_slice(&data[start..end]);
        *offset += n as u64;
        n as isize
    }

    fn write(&self, buf: &[u8]) -> isize {
        if !self.writable { return -1; }
        let mut data = self.data.lock();
        let mut offset = self.offset.lock();
        let pos = if self.append { data.len() } else { *offset as usize };
        if pos + buf.len() > data.len() {
            data.resize(pos + buf.len(), 0);
        }
        data[pos..pos + buf.len()].copy_from_slice(buf);
        *offset = (pos + buf.len()) as u64;

        // 更新文件系统中的数据
        let fs = self.get_fs();
        let mut nodes = fs.nodes.lock();
        if let Some(FsNode::File(f)) = nodes.get_mut(&self.path) {
            f.data = data.clone();
        }
        buf.len() as isize
    }

    fn stat(&self) -> FileStat {
        let data = self.data.lock();
        let now = crate::timer::get_time_ms() as i64 / 1000;
        FileStat {
            st_dev: 1,
            st_ino: self.ino,
            st_mode: super::inode::InodeType::Regular.mode_bits() | 0o644,
            st_nlink: 1,
            st_size: data.len() as i64,
            st_blksize: 4096,
            st_blocks: ((data.len() + 511) / 512) as i64,
            st_atime: now,
            st_mtime: now,
            st_ctime: now,
            ..Default::default()
        }
    }

    fn is_readable(&self) -> bool { self.readable }
    fn is_writable(&self) -> bool { self.writable }

    fn seek(&self, offset: i64, whence: i32) -> i64 {
        let data = self.data.lock();
        let mut current = self.offset.lock();
        let new_offset = match whence {
            0 => offset,                              // SEEK_SET
            1 => *current as i64 + offset,           // SEEK_CUR
            2 => data.len() as i64 + offset,         // SEEK_END
            _ => return -1,
        };
        if new_offset < 0 { return -1; }
        *current = new_offset as u64;
        new_offset
    }

    fn get_offset(&self) -> u64 {
        *self.offset.lock()
    }

    fn truncate(&self, size: u64) -> isize {
        let mut data = self.data.lock();
        data.resize(size as usize, 0);
        let fs = self.get_fs();
        let mut nodes = fs.nodes.lock();
        if let Some(FsNode::File(f)) = nodes.get_mut(&self.path) {
            f.data = data.clone();
        }
        0
    }

    fn get_path(&self) -> Option<String> {
        Some(self.path.clone())
    }

    fn get_flags(&self) -> i32 {
        *self.flags.lock()
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> isize {
        if !self.readable { return -1; }
        let data = self.data.lock();
        let start = offset as usize;
        if start >= data.len() { return 0; }
        let end = (start + buf.len()).min(data.len());
        let n = end - start;
        buf[..n].copy_from_slice(&data[start..end]);
        n as isize
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> isize {
        if !self.writable { return -1; }
        let mut data = self.data.lock();
        let pos = offset as usize;
        if pos + buf.len() > data.len() {
            data.resize(pos + buf.len(), 0);
        }
        data[pos..pos + buf.len()].copy_from_slice(buf);
        let fs = self.get_fs();
        let mut nodes = fs.nodes.lock();
        if let Some(FsNode::File(f)) = nodes.get_mut(&self.path) {
            f.data = data.clone();
        }
        buf.len() as isize
    }

    fn set_flags(&self, flags: i32) {
        *self.flags.lock() = flags;
    }

    fn set_nonblock(&self, nonblock: bool) {
        *self.nonblock.lock() = nonblock;
    }

    fn is_nonblock(&self) -> bool {
        *self.nonblock.lock()
    }
}

/// TmpFs 目录描述符
pub struct TmpDir {
    path: String,
    fs_ptr: Arc<usize>,
    offset: Mutex<usize>,
}

unsafe impl Send for TmpDir {}
unsafe impl Sync for TmpDir {}

impl FileDescriptor for TmpDir {
    fn read(&self, _buf: &mut [u8]) -> isize { -1 }
    fn write(&self, _buf: &[u8]) -> isize { -1 }
    fn stat(&self) -> FileStat {
        let now = crate::timer::get_time_ms() as i64 / 1000;
        FileStat {
            st_mode: super::inode::InodeType::Directory.mode_bits() | 0o755,
            st_nlink: 2,
            st_blksize: 4096,
            st_atime: now, st_mtime: now, st_ctime: now,
            ..Default::default()
        }
    }
    fn is_readable(&self) -> bool { false }
    fn is_writable(&self) -> bool { false }
    fn is_directory(&self) -> bool { true }

    fn readdir(&self) -> Option<Vec<DirEntry>> {
        let fs = unsafe { &*(*self.fs_ptr as *const TmpFs) };
        fs.readdir(&self.path)
    }

    fn getdents(&self, buf: &mut [u8]) -> isize {
        let entries = match self.readdir() {
            Some(e) => e,
            None => return -1,
        };
        let mut offset = *self.offset.lock();
        let mut written = 0;

        for (i, entry) in entries.iter().enumerate() {
            if i < offset { continue; }

            // linux_dirent64 结构：
            // u64 d_ino, i64 d_off, u16 d_reclen, u8 d_type, char d_name[]
            let name_bytes = entry.name.as_bytes();
            let reclen = (8 + 8 + 2 + 1 + name_bytes.len() + 1 + 7) & !7;

            if written + reclen > buf.len() { break; }

            let b = &mut buf[written..written + reclen];
            // d_ino
            b[0..8].copy_from_slice(&entry.inode.to_le_bytes());
            // d_off
            let next_off = (i + 1) as i64;
            b[8..16].copy_from_slice(&next_off.to_le_bytes());
            // d_reclen
            b[16..18].copy_from_slice(&(reclen as u16).to_le_bytes());
            // d_type
            b[18] = entry.file_type;
            // d_name
            b[19..19 + name_bytes.len()].copy_from_slice(name_bytes);
            b[19 + name_bytes.len()] = 0;

            written += reclen;
            offset += 1;
        }

        *self.offset.lock() = offset;
        written as isize
    }

    fn get_path(&self) -> Option<String> {
        Some(self.path.clone())
    }
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => { parts.pop(); }
            p => parts.push(p),
        }
    }
    let mut result = String::from("/");
    result.push_str(&parts.join("/"));
    result
}

fn parent_dir(path: &str) -> &str {
    if let Some(pos) = path.rfind('/') {
        if pos == 0 { "/" } else { &path[..pos] }
    } else {
        "/"
    }
}
