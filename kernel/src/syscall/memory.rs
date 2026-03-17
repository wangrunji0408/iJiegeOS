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

    log::debug!("mmap: addr={:#x}, len={}, prot={}, flags={:#x}, fd={}, offset={}", addr, len, prot, flags, fd, offset);

    // MAP_ANONYMOUS
    if flags & 0x20 != 0 {
        // 匿名映射
        let start = inner.memory_set.mmap(addr, len, prot as usize);
        log::debug!("mmap anon: start={:#x}", start);
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
    if offset as usize >= file_size {
        // 映射超出文件，只分配匿名内存
        let start = inner.memory_set.mmap(addr, len, prot as usize);
        log::debug!("mmap file (empty): start={:#x}", start);
        return start as i64;
    }
    let read_len = (file_size - offset as usize).min(len);

    let start = inner.memory_set.mmap(addr, len, prot as usize);
    log::debug!("mmap file: start={:#x}, file_size={}, read_len={}", start, file_size, read_len);

    if read_len > 0 {
        let mut data = alloc::vec![0u8; read_len];
        file.read_at(offset as u64, &mut data);

        // 将数据写入映射区域
        let tok = inner.memory_set.token();
        drop(inner);  // 释放锁以避免死锁
        let bufs = crate::mm::translated_byte_buffer(tok, start as *mut u8, read_len);
        let mut off = 0;
        for b in bufs {
            let n = b.len().min(read_len - off);
            b[..n].copy_from_slice(&data[off..off + n]);
            off += n;
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
