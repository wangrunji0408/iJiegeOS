/// 文件相关系统调用

use alloc::string::String;
use alloc::sync::Arc;
use crate::fs::{FileDescriptor, FileStat, DirEntry};
use crate::mm::{translated_byte_buffer, translated_str, translated_refmut};
use crate::task::current_task;
use super::errno::*;

/// 获取当前用户 token
fn token() -> usize {
    crate::task::current_user_token()
}

/// 将用户指针转为字符串
fn user_str(ptr: *const u8) -> String {
    translated_str(token(), ptr)
}

/// resolve path (handle AT_FDCWD = -100)
fn resolve_path(dirfd: i32, path: &str) -> String {
    const AT_FDCWD: i32 = -100;
    if path.starts_with('/') {
        return String::from(path);
    }
    if dirfd == AT_FDCWD {
        let task = current_task().unwrap();
        let inner = task.inner_exclusive_access();
        if path.is_empty() || path == "." {
            return inner.cwd.clone();
        }
        alloc::format!("{}/{}", inner.cwd, path)
    } else {
        // 从 fd 获取目录路径
        let task = current_task().unwrap();
        let inner = task.inner_exclusive_access();
        if let Some(fd) = inner.get_fd(dirfd as usize) {
            if let Some(dir_path) = fd.get_path() {
                return alloc::format!("{}/{}", dir_path, path);
            }
        }
        String::from(path)
    }
}

pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let mut user_buf = translated_byte_buffer(token(), buf, len);
    let mut total = 0i64;
    for slice in user_buf.iter_mut() {
        let n = file.read(slice);
        if n < 0 { return n as i64; }
        total += n as i64;
        if (n as usize) < slice.len() { break; }
    }
    total
}

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let user_buf = translated_byte_buffer(token(), buf, len);
    let mut total = 0i64;
    for slice in user_buf.iter() {
        let n = file.write(slice);
        if n < 0 { return n as i64; }
        total += n as i64;
        if (n as usize) < slice.len() { break; }
    }
    total
}

pub fn sys_openat(dirfd: i32, path: *const u8, flags: i32, mode: u32) -> i64 {
    let path_str = user_str(path);
    let abs_path = resolve_path(dirfd, &path_str);

    let file = crate::fs::open(&abs_path, flags, mode);
    let file = match file {
        Some(f) => f,
        None => {
            log::debug!("openat: not found: {}", abs_path);
            return ENOENT;
        }
    };

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let fd = inner.alloc_fd();
    inner.fd_table[fd] = Some(file);
    fd as i64
}

pub fn sys_close(fd: usize) -> i64 {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() { return EBADF; }
    if inner.fd_table[fd].is_none() { return EBADF; }
    inner.fd_table[fd] = None;
    0
}

pub fn sys_fstat(fd: usize, stat: *mut FileStat) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let file_stat = file.stat();
    let user_stat = translated_refmut(token(), stat);
    *user_stat = file_stat;
    0
}

pub fn sys_newfstatat(dirfd: i32, path: *const u8, stat: *mut FileStat, flags: i32) -> i64 {
    let path_str = user_str(path);
    let abs_path = resolve_path(dirfd, &path_str);

    let file_stat = match crate::fs::stat(&abs_path) {
        Some(s) => s,
        None => return ENOENT,
    };

    if !stat.is_null() {
        let user_stat = translated_refmut(token(), stat);
        *user_stat = file_stat;
    }
    0
}

pub fn sys_lseek(fd: usize, offset: i64, whence: i32) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);
    file.seek(offset, whence)
}

pub fn sys_readv(fd: usize, iov: usize, iovcnt: usize) -> i64 {
    // struct iovec { void *iov_base, size_t iov_len }
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let mut total = 0i64;
    let tok = token();
    for i in 0..iovcnt {
        let iov_entry = iov + i * 16;
        let base = unsafe { *(translated_refmut(tok, iov_entry as *mut usize)) };
        let len = unsafe { *(translated_refmut(tok, (iov_entry + 8) as *mut usize)) };
        if len == 0 { continue; }
        let mut bufs = translated_byte_buffer(tok, base as *mut u8, len);
        for buf in bufs.iter_mut() {
            let n = file.read(buf);
            if n < 0 { return n as i64; }
            total += n as i64;
        }
    }
    total
}

pub fn sys_writev(fd: usize, iov: usize, iovcnt: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let mut total = 0i64;
    let tok = token();
    for i in 0..iovcnt {
        let iov_entry = iov + i * 16;
        let base = unsafe { *(translated_refmut(tok, iov_entry as *mut usize)) };
        let len = unsafe { *(translated_refmut(tok, (iov_entry + 8) as *mut usize)) };
        if len == 0 { continue; }
        let bufs = translated_byte_buffer(tok, base as *mut u8, len);
        for buf in bufs.iter() {
            let n = file.write(buf);
            if n < 0 { return n as i64; }
            total += n as i64;
        }
    }
    total
}

pub fn sys_pread64(fd: usize, buf: *mut u8, len: usize, offset: i64) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let mut bufs = translated_byte_buffer(token(), buf, len);
    let mut total = 0i64;
    for buf in bufs.iter_mut() {
        let n = file.read_at(offset as u64 + total as u64, buf);
        if n < 0 { return n as i64; }
        total += n as i64;
    }
    total
}

pub fn sys_pwrite64(fd: usize, buf: *const u8, len: usize, offset: i64) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let bufs = translated_byte_buffer(token(), buf, len);
    let mut total = 0i64;
    for buf in bufs.iter() {
        let n = file.write_at(offset as u64 + total as u64, buf);
        if n < 0 { return n as i64; }
        total += n as i64;
    }
    total
}

pub fn sys_dup(fd: usize) -> i64 {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(file);
    new_fd as i64
}

pub fn sys_dup3(old_fd: usize, new_fd: usize, flags: i32) -> i64 {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let file = match inner.get_fd(old_fd) {
        Some(f) => f,
        None => return EBADF,
    };
    // 扩展 fd_table 到足够大
    while inner.fd_table.len() <= new_fd {
        inner.fd_table.push(None);
    }
    inner.fd_table[new_fd] = Some(file);
    new_fd as i64
}

pub fn sys_pipe2(fds: *mut i32, flags: i32) -> i64 {
    let (read_end, write_end) = crate::fs::create_pipe();

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(read_end);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(write_end);
    drop(inner);

    let tok = token();
    let user_fds = translated_refmut(tok, fds);
    *user_fds = read_fd as i32;
    let user_fds1 = translated_refmut(tok, unsafe { fds.add(1) });
    *user_fds1 = write_fd as i32;

    0
}

pub fn sys_fcntl(fd: usize, cmd: i32, arg: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    match cmd {
        0  => { // F_DUPFD
            let task = current_task().unwrap();
            let mut inner = task.inner_exclusive_access();
            let new_fd = inner.alloc_fd();
            // 确保 new_fd >= arg
            while new_fd < arg {
                let new_fd2 = inner.alloc_fd();
                if new_fd2 >= arg {
                    inner.fd_table[new_fd2] = Some(file.clone());
                    return new_fd2 as i64;
                }
            }
            inner.fd_table[new_fd] = Some(file);
            new_fd as i64
        }
        1  => file.get_flags() as i64,  // F_GETFD
        2  => { file.set_flags(arg as i32); 0 },  // F_SETFD
        3  => file.get_flags() as i64,  // F_GETFL
        4  => { file.set_flags(arg as i32); 0 },  // F_SETFL
        // F_GETLK, F_SETLK, F_SETLKW
        5 | 6 | 7 => 0,
        _ => file.fcntl(cmd, arg),
    }
}

pub fn sys_ioctl(fd: usize, request: u64, arg: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);
    file.ioctl(request, arg)
}

pub fn sys_getdents64(fd: usize, buf: *mut u8, len: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let mut kernel_buf = alloc::vec![0u8; len];
    let n = file.getdents(&mut kernel_buf);
    if n <= 0 { return n; }

    let user_bufs = translated_byte_buffer(token(), buf, n as usize);
    let mut offset = 0;
    for user_buf in user_bufs {
        user_buf.copy_from_slice(&kernel_buf[offset..offset + user_buf.len()]);
        offset += user_buf.len();
    }
    n
}

pub fn sys_readlinkat(dirfd: i32, path: *const u8, buf: *mut u8, size: usize) -> i64 {
    let path_str = user_str(path);
    let abs_path = resolve_path(dirfd, &path_str);

    // /proc/self/exe 特殊处理
    if abs_path == "/proc/self/exe" {
        let link = b"/usr/sbin/nginx";
        let n = link.len().min(size);
        let bufs = translated_byte_buffer(token(), buf, n);
        let mut offset = 0;
        for b in bufs {
            let end = (offset + b.len()).min(n);
            b[..end-offset].copy_from_slice(&link[offset..end]);
            offset = end;
        }
        return n as i64;
    }

    if let Some(target) = crate::fs::readlink(&abs_path) {
        let bytes = target.as_bytes();
        let n = bytes.len().min(size);
        let bufs = translated_byte_buffer(token(), buf, n);
        let mut offset = 0;
        for b in bufs {
            let end = (offset + b.len()).min(n);
            b.copy_from_slice(&bytes[offset..offset + b.len()]);
            offset += b.len();
        }
        return n as i64;
    }

    // 尝试作为普通文件读 stat
    match crate::fs::stat(&abs_path) {
        Some(_) => EINVAL,  // 不是符号链接
        None => ENOENT,
    }
}

pub fn sys_faccessat(dirfd: i32, path: *const u8, mode: i32, flags: i32) -> i64 {
    let path_str = user_str(path);
    let abs_path = resolve_path(dirfd, &path_str);
    match crate::fs::stat(&abs_path) {
        Some(_) => 0,
        None => ENOENT,
    }
}

pub fn sys_mkdirat(dirfd: i32, path: *const u8, mode: u32) -> i64 {
    let path_str = user_str(path);
    let abs_path = resolve_path(dirfd, &path_str);
    // TODO: 通过 VFS 创建目录
    0
}

pub fn sys_unlinkat(dirfd: i32, path: *const u8, flags: i32) -> i64 {
    let path_str = user_str(path);
    let abs_path = resolve_path(dirfd, &path_str);
    // TODO: 通过 VFS 删除文件
    0
}

pub fn sys_renameat(olddirfd: i32, oldpath: *const u8, newdirfd: i32, newpath: *const u8) -> i64 {
    let old_str = user_str(oldpath);
    let new_str = user_str(newpath);
    // TODO
    0
}

pub fn sys_ftruncate(fd: usize, size: i64) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);
    file.truncate(size as u64)
}

pub fn sys_flock(fd: usize, how: i32) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);
    file.flock(how)
}

pub fn sys_sendfile(out_fd: usize, in_fd: usize, offset: *mut i64, count: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let in_file = match inner.get_fd(in_fd) {
        Some(f) => f,
        None => return EBADF,
    };
    let out_file = match inner.get_fd(out_fd) {
        Some(f) => f,
        None => return EBADF,
    };
    drop(inner);

    let mut buf = alloc::vec![0u8; count.min(65536)];
    let n = in_file.read(&mut buf);
    if n <= 0 { return n; }
    out_file.write(&buf[..n as usize])
}

pub fn sys_getcwd(buf: *mut u8, size: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let cwd = inner.cwd.clone();
    drop(inner);

    let bytes = cwd.as_bytes();
    let n = (bytes.len() + 1).min(size);
    let bufs = translated_byte_buffer(token(), buf, n);
    let mut offset = 0;
    for b in bufs {
        let end = (offset + b.len()).min(bytes.len());
        b[..end-offset.min(end-offset)].copy_from_slice(&bytes[offset..end.min(bytes.len())]);
        if end >= bytes.len() && b.len() > end - offset {
            b[end - offset] = 0;
        }
        offset += b.len();
    }
    buf as i64
}

pub fn sys_chdir(path: *const u8) -> i64 {
    let path_str = user_str(path);
    match crate::fs::stat(&path_str) {
        Some(s) if s.st_mode & 0o170000 == 0o040000 => {
            let task = current_task().unwrap();
            let mut inner = task.inner_exclusive_access();
            inner.cwd = path_str;
            0
        }
        Some(_) => ENOTDIR,
        None => ENOENT,
    }
}

pub fn sys_fchdir(fd: usize) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let file = match inner.get_fd(fd) {
        Some(f) => f,
        None => return EBADF,
    };
    if !file.is_directory() { return ENOTDIR; }
    if let Some(path) = file.get_path() {
        drop(inner);
        drop(file);
        let task = current_task().unwrap();
        let mut inner = task.inner_exclusive_access();
        inner.cwd = path;
        0
    } else {
        EBADF
    }
}

pub fn sys_fchmod(fd: usize, mode: u32) -> i64 { 0 }
pub fn sys_fchmodat(dirfd: i32, path: *const u8, mode: u32) -> i64 { 0 }

pub fn sys_statfs(path: *const u8, buf: *mut u8) -> i64 {
    // 返回一个假的 statfs 结构
    // struct statfs64: f_type, f_bsize, f_blocks, f_bfree, f_bavail, f_files, f_ffree, f_fsid, f_namelen, f_frsize, f_flags
    let tok = token();
    let buf_ptr = translated_refmut(tok, buf as *mut u64);
    // f_type = TMPFS_MAGIC = 0x01021994
    unsafe {
        let p = buf as *mut u64;
        *p = 0x01021994;       // f_type
        *p.add(1) = 4096;      // f_bsize
        *p.add(2) = 1024*1024; // f_blocks
        *p.add(3) = 512*1024;  // f_bfree
        *p.add(4) = 512*1024;  // f_bavail
        *p.add(5) = 65536;     // f_files
        *p.add(6) = 32768;     // f_ffree
        // f_fsid (16 bytes)
        *p.add(7) = 0;
        *p.add(8) = 0;
        *p.add(9) = 255;       // f_namelen
        *p.add(10) = 4096;     // f_frsize
        *p.add(11) = 0;        // f_flags
    }
    0
}

pub fn sys_symlinkat(target: *const u8, newdirfd: i32, linkpath: *const u8) -> i64 {
    0  // stub
}

pub fn sys_linkat(olddirfd: i32, oldpath: *const u8, newdirfd: i32, newpath: *const u8, flags: i32) -> i64 {
    0  // stub
}

pub fn sys_close_range(first: u32, last: u32, flags: i32) -> i64 {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let len = inner.fd_table.len();
    for fd in (first as usize)..=(last as usize).min(len - 1) {
        inner.fd_table[fd] = None;
    }
    0
}

pub fn sys_readlink(path: *const u8, buf: *mut u8, size: usize) -> i64 {
    sys_readlinkat(-100, path, buf, size)
}
