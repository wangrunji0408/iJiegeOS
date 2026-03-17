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
        let start = inner.memory_set.mmap(addr, len, prot as usize);
        return start as i64;
    }

    // 文件映射（懒加载）
    if fd < 0 { return EINVAL; }

    let file = match inner.get_fd(fd as usize) {
        Some(f) => f,
        None => return EBADF,
    };

    let stat = file.stat();
    let file_size = stat.st_size as usize;
    if offset as usize >= file_size {
        // 映射超出文件，只分配匿名内存
        let start = inner.memory_set.mmap(addr, len, prot as usize);
        return start as i64;
    }

    // 懒加载：不立即拷贝文件内容，只注册映射区域
    let start = inner.memory_set.mmap_file(addr, len, prot as usize, file, offset as usize);
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
