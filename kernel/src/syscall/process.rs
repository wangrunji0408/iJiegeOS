/// 进程管理系统调用

use alloc::sync::Arc;
use crate::arch::trap::TrapContext;
use crate::task::{Task, TaskState, current_task, add_task, TASK_MANAGER};
use crate::mm::translated_str;
use super::errno::*;

fn token() -> usize {
    crate::task::current_user_token()
}

pub fn sys_getpid() -> i64 {
    current_task().map(|t| t.pid.0 as i64).unwrap_or(1)
}

pub fn sys_getppid() -> i64 {
    current_task().and_then(|t| {
        t.inner_exclusive_access().parent.as_ref()?.upgrade().map(|p| p.pid.0 as i64)
    }).unwrap_or(0)
}

pub fn sys_gettid() -> i64 {
    sys_getpid()  // 单线程简化
}

pub fn sys_getuid() -> i64 { 0 }
pub fn sys_getgid() -> i64 { 0 }

pub fn sys_getresuid(ruid: usize, euid: usize, suid: usize) -> i64 {
    let tok = token();
    *crate::mm::translated_refmut(tok, ruid as *mut u32) = 0;
    *crate::mm::translated_refmut(tok, euid as *mut u32) = 0;
    *crate::mm::translated_refmut(tok, suid as *mut u32) = 0;
    0
}

pub fn sys_getresgid(rgid: usize, egid: usize, sgid: usize) -> i64 {
    sys_getresuid(rgid, egid, sgid)
}

pub fn sys_getgroups(size: i32, list: usize) -> i64 {
    if size == 0 { return 1; }
    if list != 0 {
        *crate::mm::translated_refmut(token(), list as *mut u32) = 0;
    }
    1
}

pub fn sys_setpgid(pid: i32, pgid: i32) -> i64 { 0 }

pub fn sys_getpgid(pid: i32) -> i64 {
    if pid == 0 {
        current_task().map(|t| t.inner_exclusive_access().pgid as i64).unwrap_or(1)
    } else {
        pid as i64
    }
}

pub fn sys_getsid(pid: i32) -> i64 {
    if pid == 0 {
        current_task().map(|t| t.inner_exclusive_access().sid as i64).unwrap_or(1)
    } else {
        pid as i64
    }
}

pub fn sys_setsid() -> i64 {
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        inner.sid = task.pid.0;
        inner.pgid = task.pid.0;
        task.pid.0 as i64
    } else {
        EPERM
    }
}

pub fn sys_exit(exit_code: i32) -> i64 {
    crate::task::exit_current_and_run_next(exit_code as usize);
    unreachable!()
}

pub fn sys_exit_group(exit_code: i32) -> i64 {
    sys_exit(exit_code)
}

pub fn sys_clone(flags: usize, child_sp: usize, ptid: usize, ctid: usize, newtls: usize, ctx: &mut TrapContext) -> i64 {
    let current = current_task().expect("no current task");
    let child_task = fork_task(&current, flags, child_sp, ptid, ctid, newtls, ctx);
    let child_pid = child_task.pid.0;
    add_task(Arc::new(child_task));
    child_pid as i64
}

fn fork_task(parent: &Arc<Task>, flags: usize, child_sp: usize, ptid: usize, ctid: usize, newtls: usize, parent_ctx: &TrapContext) -> Task {
    let pid = crate::task::pid::PID_ALLOCATOR.alloc();
    let kernel_stack = crate::task::task::KernelStack::new();
    let kernel_sp = kernel_stack.top();

    // 复制地址空间
    let parent_inner = parent.inner_exclusive_access();
    let mut memory_set = crate::mm::MemorySet::fork_from(&parent_inner.memory_set);

    // TrapContext 在内核栈上，不需要 ppn 映射
    // 在内核栈上保留一个 TrapContext 空间
    let trap_cx_addr = kernel_sp - core::mem::size_of::<TrapContext>();
    let trap_cx = unsafe { &mut *(trap_cx_addr as *mut TrapContext) };

    // 复制父进程的 TrapContext
    *trap_cx = *parent_ctx;
    // 更新子进程的用户页表 satp
    trap_cx.user_satp = memory_set.token();
    trap_cx.kernel_satp = riscv::register::satp::read().bits();
    // 子进程 fork 返回 0
    trap_cx.set_return_value(0);
    // 如果指定了子进程栈
    if child_sp != 0 {
        trap_cx.set_sp(child_sp);
    }

    let task_cx = crate::task::context::TaskContext::goto_trap_return(trap_cx_addr);

    // 复制文件描述符
    let fd_table = parent_inner.fd_table.clone();
    let cwd = parent_inner.cwd.clone();
    let brk = memory_set.brk;
    let brk_start = memory_set.brk_start;

    // 如果需要设置子进程 TID（ctid 需要是有效用户空间地址）
    if ctid > 0x1000 {  // 跳过无效地址（如太小的值）
        let tok = memory_set.token();
        if let Some(pa) = crate::mm::PageTable::from_token(tok).translate_va(crate::mm::VirtAddr::from(ctid)) {
            let user_tid = pa.get_mut::<i32>();
            *user_tid = pid.0;
        }
    }

    let inner = crate::task::task::TaskInner {
        state: TaskState::Ready,
        task_cx,
        memory_set,
        trap_cx_addr,
        parent: Some(Arc::downgrade(parent)),
        children: alloc::vec::Vec::new(),
        exit_code: 0,
        pending_signals: alloc::vec::Vec::new(),
        fd_table,
        cwd,
        heap_start: brk_start,
        heap_end: brk,
        uid: parent_inner.uid,
        gid: parent_inner.gid,
        euid: parent_inner.euid,
        egid: parent_inner.egid,
        pgid: parent_inner.pgid,
        sid: parent_inner.sid,
        robust_list: 0,
        clear_child_tid: ctid,
        set_child_tid: ptid,
        rlimits: parent_inner.rlimits,
        tid: pid,
    };

    drop(parent_inner);

    Task {
        pid,
        kernel_stack,
        inner: spin::Mutex::new(inner),
    }
}

pub fn sys_execve(path: *const u8, argv: *const usize, envp: *const usize, ctx: &mut TrapContext) -> i64 {
    let tok = token();
    let path_str = translated_str(tok, path);

    // 读取参数
    let mut args: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    if !argv.is_null() {
        let mut i = 0;
        loop {
            let arg_ptr = *crate::mm::translated_ref(tok, unsafe { argv.add(i) });
            if arg_ptr == 0 { break; }
            args.push(translated_str(tok, arg_ptr as *const u8));
            i += 1;
        }
    }

    // 读取环境变量
    let mut envs: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    if !envp.is_null() {
        let mut i = 0;
        loop {
            let env_ptr = *crate::mm::translated_ref(tok, unsafe { envp.add(i) });
            if env_ptr == 0 { break; }
            envs.push(translated_str(tok, env_ptr as *const u8));
            i += 1;
        }
    }

    log::info!("execve: {}", path_str);

    // 打开并读取 ELF
    let elf_file = match crate::fs::open(&path_str, 0, 0) {
        Some(f) => f,
        None => return ENOENT,
    };

    let stat = elf_file.stat();
    let size = stat.st_size as usize;
    let mut elf_data = alloc::vec![0u8; size];
    elf_file.read(&mut elf_data);

    // 替换当前进程地址空间
    let task = current_task().expect("no task");
    let mut inner = task.inner_exclusive_access();

    // 创建新地址空间
    let (mut new_ms, user_sp, entry) = crate::mm::MemorySet::new_user(&elf_data);

    // 在新地址空间中设置栈（push 参数）
    let user_sp = setup_stack(&mut new_ms, user_sp, &args, &envs);

    // 替换地址空间
    inner.memory_set = new_ms;

    // 更新 TrapContext
    let trap_cx_addr = task.kernel_stack.top() - core::mem::size_of::<TrapContext>();
    let trap_cx = unsafe { &mut *(trap_cx_addr as *mut TrapContext) };
    *trap_cx = TrapContext::new(entry, user_sp);
    trap_cx.user_satp = inner.memory_set.token();
    trap_cx.kernel_satp = riscv::register::satp::read().bits();
    inner.trap_cx_addr = trap_cx_addr;

    // 更新调度上下文（使新地址空间生效）
    inner.task_cx = crate::task::context::TaskContext::goto_trap_return(trap_cx_addr);

    // 更新 ctx 指向的内存
    *ctx = *trap_cx;

    0
}

fn setup_stack(ms: &mut crate::mm::MemorySet, mut sp: usize, args: &[alloc::string::String], envs: &[alloc::string::String]) -> usize {
    // 将参数字符串压栈
    let tok = ms.token();

    // 推入参数字符串
    let mut arg_ptrs: alloc::vec::Vec<usize> = alloc::vec::Vec::new();
    for arg in args.iter().rev() {
        let bytes = arg.as_bytes();
        sp -= bytes.len() + 1;
        sp &= !7;  // 对齐
        let bufs = crate::mm::translated_byte_buffer(tok, sp as *mut u8, bytes.len() + 1);
        let mut off = 0;
        for b in bufs {
            let end = (off + b.len()).min(bytes.len());
            b[..end-off].copy_from_slice(&bytes[off..end]);
            if b.len() > end - off {
                b[end - off] = 0;
            }
            off += b.len();
        }
        arg_ptrs.push(sp);
    }
    arg_ptrs.reverse();

    // 对齐
    sp &= !15;

    // 推入 envp NULL 和 argc
    // 简化：只推入 argc 和 argv 数组
    sp -= 8;  // NULL terminator for argv
    *crate::mm::translated_refmut(tok, sp as *mut usize) = 0;

    for &ptr in arg_ptrs.iter().rev() {
        sp -= 8;
        *crate::mm::translated_refmut(tok, sp as *mut usize) = ptr;
    }

    let argv_ptr = sp;

    sp -= 8;  // argc
    *crate::mm::translated_refmut(tok, sp as *mut usize) = args.len();

    sp
}

pub fn sys_wait4(pid: i32, wstatus: usize, options: i32) -> i64 {
    // 简化的 wait4
    // 查找子进程
    let current = current_task().unwrap();
    let my_pid = current.pid.0;

    loop {
        let inner = current.inner_exclusive_access();
        let children = inner.children.clone();
        drop(inner);

        for child in children {
            let child_inner = child.inner_exclusive_access();
            if let TaskState::Zombie(code) = child_inner.state {
                let child_pid = child.pid.0;
                if pid == -1 || pid == child_pid {
                    // 找到退出的子进程
                    log::warn!("[pid={}] wait4: found zombie child pid={}, code={}", my_pid, child_pid, code);
                    drop(child_inner);
                    // 从子进程列表中移除
                    let mut inner = current.inner_exclusive_access();
                    inner.children.retain(|c| c.pid != child.pid);

                    // 设置 wstatus
                    if wstatus != 0 {
                        let tok = crate::task::current_user_token();
                        let status = crate::mm::translated_refmut(tok, wstatus as *mut i32);
                        *status = code << 8;  // WIFEXITED + exit code
                    }

                    return child_pid as i64;
                }
            }
        }

        // 检查是否有子进程
        let inner = current.inner_exclusive_access();
        if inner.children.is_empty() {
            log::warn!("[pid={}] wait4: no children, returning ECHILD", my_pid);
            return ECHILD;
        }
        drop(inner);

        // WNOHANG
        if options & 1 != 0 {
            return 0;
        }

        // 让出 CPU 等待
        log::warn!("[pid={}] wait4: no zombie child, yielding...", my_pid);
        crate::task::suspend_current_and_run_next();
    }
}

pub fn sys_set_tid_address(tidptr: usize) -> i64 {
    if let Some(task) = current_task() {
        task.inner_exclusive_access().clear_child_tid = tidptr;
    }
    sys_gettid()
}

pub fn sys_prctl(option: i32, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> i64 {
    match option {
        15 => {  // PR_SET_NAME
            // 设置进程名（忽略）
            0
        }
        16 => 0,  // PR_GET_NAME
        _ => 0,
    }
}

pub fn sys_getrlimit(resource: u32, rlim: *mut u64) -> i64 {
    let tok = token();
    let ptr = crate::mm::translated_refmut(tok, rlim);
    *ptr = u64::MAX;
    let ptr2 = crate::mm::translated_refmut(tok, unsafe { rlim.add(1) });
    *ptr2 = u64::MAX;
    0
}

pub fn sys_setrlimit(resource: u32, rlim: *const u64) -> i64 { 0 }

pub fn sys_prlimit64(pid: i32, resource: u32, new_limit: *const u64, old_limit: *mut u64) -> i64 {
    if !old_limit.is_null() {
        let tok = token();
        *crate::mm::translated_refmut(tok, old_limit) = u64::MAX;
        *crate::mm::translated_refmut(tok, unsafe { old_limit.add(1) }) = u64::MAX;
    }
    0
}

pub fn sys_getrusage(who: i32, usage: usize) -> i64 {
    // 返回空的 rusage 结构
    // struct rusage 大小为 144 字节
    if usage != 0 {
        let tok = token();
        let bufs = crate::mm::translated_byte_buffer(tok, usage as *mut u8, 144);
        for b in bufs {
            for byte in b.iter_mut() {
                *byte = 0;
            }
        }
    }
    0
}

pub fn sys_umask(mask: u32) -> i64 {
    0o022  // 返回旧的 umask
}

pub fn sys_personality(persona: u32) -> i64 {
    0  // 返回 POSIX
}
