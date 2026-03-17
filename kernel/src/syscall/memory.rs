/// 内存管理系统调用

use crate::task::current_task;
use super::errno::*;

pub fn sys_brk(new_brk: usize) -> i64 {
    let task = current_task().expect("no task");
    let mut inner = task.inner_exclusive_access();
    let result = if new_brk == 0 {
        inner.memory_set.brk
    } else {
        inner.memory_set.set_brk(new_brk)
    };
    result as i64
}

pub fn sys_mmap(addr: usize, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> i64 {
    if len == 0 { return EINVAL; }

    let task = current_task().expect("no task");
    let mut inner = task.inner_exclusive_access();

    // MAP_ANONYMOUS
    if flags & 0x20 != 0 {
        // 匿名映射
        let start = inner.memory_set.mmap(addr, len, prot as usize);
        return start as i64;
    }

    // 文件映射
    if fd < 0 { return EINVAL; }

    let file = match inner.get_fd(fd as usize) {
        Some(f) => f,
        None => return EBADF,
    };

    // 读取文件内容到内存
    let stat = file.stat();
    let file_size = stat.st_size as usize;
    let read_len = (offset as usize + len).min(file_size) - offset as usize;

    let start = inner.memory_set.mmap(addr, len, prot as usize);

    if read_len > 0 {
        let mut data = alloc::vec![0u8; read_len];
        file.read_at(offset as u64, &mut data);

        // 将数据写入映射区域
        let tok = inner.memory_set.token();
        let bufs = crate::mm::translated_byte_buffer(tok, start as *mut u8, read_len);
        let mut off = 0;
        for b in bufs {
            let end = (off + b.len()).min(read_len);
            b[..end-off].copy_from_slice(&data[off..end]);
            off += b.len();
        }
    }

    start as i64
}

pub fn sys_munmap(addr: usize, len: usize) -> i64 {
    let task = current_task().expect("no task");
    let mut inner = task.inner_exclusive_access();
    inner.memory_set.munmap(addr, len);
    0
}

pub fn sys_mprotect(addr: usize, len: usize, prot: i32) -> i64 {
    let task = current_task().expect("no task");
    let mut inner = task.inner_exclusive_access();
    inner.memory_set.mprotect(addr, len, prot as usize);
    0
}
