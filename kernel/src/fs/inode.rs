/// Inode 类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InodeType {
    Regular,
    Directory,
    Symlink,
    Pipe,
    Socket,
    CharDevice,
    BlockDevice,
}

impl InodeType {
    pub fn mode_bits(&self) -> u32 {
        match self {
            InodeType::Regular => 0o100000,
            InodeType::Directory => 0o040000,
            InodeType::Symlink => 0o120000,
            InodeType::Pipe => 0o010000,
            InodeType::Socket => 0o140000,
            InodeType::CharDevice => 0o020000,
            InodeType::BlockDevice => 0o060000,
        }
    }

    pub fn dirent_type(&self) -> u8 {
        match self {
            InodeType::Regular => 8,
            InodeType::Directory => 4,
            InodeType::Symlink => 10,
            InodeType::CharDevice => 2,
            InodeType::BlockDevice => 6,
            InodeType::Pipe => 1,
            InodeType::Socket => 12,
        }
    }
}

pub struct Inode {
    pub ino: u64,
    pub itype: InodeType,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub nlink: u32,
    pub size: u64,
    pub atime: i64,
    pub mtime: i64,
    pub ctime: i64,
}
