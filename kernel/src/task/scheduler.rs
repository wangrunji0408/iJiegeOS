use alloc::sync::Arc;
use core::arch::asm;
use crate::task::{Task, TaskState, TaskContext, CURRENT_TASK, TASK_MANAGER};

/// 切换上下文汇编
#[naked]
unsafe extern "C" fn __switch(
    current_task_cx_ptr: *mut TaskContext,
    next_task_cx_ptr: *const TaskContext,
) {
    // 保存当前任务的 callee-saved 寄存器
    // 切换到下一个任务的寄存器
    core::arch::naked_asm!(
        // 保存 current 的 ra, sp, s0-s11
        "sd ra, 0(a0)",
        "sd sp, 8(a0)",
        "sd s0, 16(a0)",
        "sd s1, 24(a0)",
        "sd s2, 32(a0)",
        "sd s3, 40(a0)",
        "sd s4, 48(a0)",
        "sd s5, 56(a0)",
        "sd s6, 64(a0)",
        "sd s7, 72(a0)",
        "sd s8, 80(a0)",
        "sd s9, 88(a0)",
        "sd s10, 96(a0)",
        "sd s11, 104(a0)",
        // 加载 next 的 ra, sp, s0-s11
        "ld ra, 0(a1)",
        "ld sp, 8(a1)",
        "ld s0, 16(a1)",
        "ld s1, 24(a1)",
        "ld s2, 32(a1)",
        "ld s3, 40(a1)",
        "ld s4, 48(a1)",
        "ld s5, 56(a1)",
        "ld s6, 64(a1)",
        "ld s7, 72(a1)",
        "ld s8, 80(a1)",
        "ld s9, 88(a1)",
        "ld s10, 96(a1)",
        "ld s11, 104(a1)",
        "ret",
    );
}

/// 开始运行第一个任务
pub fn run_first_task() -> ! {
    let next_task = {
        let mut manager = TASK_MANAGER.lock();
        manager.pop_ready().expect("no task to run")
    };

    let mut next_inner = next_task.inner_exclusive_access();
    next_inner.state = TaskState::Running;
    let next_cx = &next_inner.task_cx as *const TaskContext;
    drop(next_inner);

    *CURRENT_TASK.lock() = Some(next_task);

    let mut idle_cx = TaskContext::new_empty();
    unsafe {
        __switch(&mut idle_cx as *mut TaskContext, next_cx);
    }
    unreachable!()
}

/// 挂起当前任务，切换到下一个就绪任务
pub fn suspend_current_and_run_next() {
    let current = {
        let guard = CURRENT_TASK.lock();
        guard.clone()
    };

    if let Some(task) = current {
        let mut inner = task.inner_exclusive_access();
        inner.state = TaskState::Ready;
        let current_cx = &mut inner.task_cx as *mut TaskContext;
        drop(inner);

        // 把当前任务放回就绪队列
        TASK_MANAGER.lock().push_ready(task.clone());

        // 找下一个任务
        if let Some(next_task) = TASK_MANAGER.lock().pop_ready() {
            let mut next_inner = next_task.inner_exclusive_access();
            next_inner.state = TaskState::Running;
            let next_cx = &next_inner.task_cx as *const TaskContext;
            drop(next_inner);

            *CURRENT_TASK.lock() = Some(next_task);

            unsafe {
                __switch(current_cx, next_cx);
            }
        }
    }
}

/// 退出当前任务，切换到下一个就绪任务
pub fn exit_current_and_run_next(exit_code: usize) {
    let current = CURRENT_TASK.lock().take();

    if let Some(task) = current {
        let mut inner = task.inner_exclusive_access();
        inner.state = TaskState::Zombie(exit_code as i32);
        inner.exit_code = exit_code as i32;
        let current_cx = &mut inner.task_cx as *mut TaskContext;

        // 通知父进程（发送 SIGCHLD）
        if let Some(parent) = inner.parent.as_ref().and_then(|p| p.upgrade()) {
            parent.inner_exclusive_access().pending_signals.push(crate::signal::Signal::SIGCHLD);
        }

        drop(inner);

        // 找下一个任务
        let next_task = TASK_MANAGER.lock().pop_ready();
        if let Some(next_task) = next_task {
            let mut next_inner = next_task.inner_exclusive_access();
            next_inner.state = TaskState::Running;
            let next_cx = &next_inner.task_cx as *const TaskContext;
            drop(next_inner);

            *CURRENT_TASK.lock() = Some(next_task);

            let mut idle_cx = TaskContext::new_empty();
            unsafe {
                __switch(&mut idle_cx as *mut TaskContext, next_cx);
            }
        } else {
            // 没有更多任务，挂起
            log::info!("No more tasks, system idle");
            loop {
                crate::arch::wait_for_interrupt();
            }
        }
    }
}

/// trap_return: 从内核态返回用户态
/// 这是新任务第一次切换时的目标
#[naked]
#[no_mangle]
pub unsafe extern "C" fn trap_return() {
    core::arch::naked_asm!(
        // 设置 stvec 到 trampoline 的 __alltraps
        "la t0, strampoline",
        // 计算 __alltraps 的虚拟地址（在 trampoline 页内的偏移）
        // TRAMPOLINE = 0x3ffff000（用户虚拟地址）
        // strampoline 在 trampoline 页开头，所以偏移 = __alltraps - strampoline + TRAMPOLINE
        "la t1, __alltraps",
        "sub t1, t1, t0",
        "li t0, 0x3ffff000",  // TRAMPOLINE 虚拟地址
        "add t0, t0, t1",
        "csrw stvec, t0",

        // 获取 TrapContext 的内核地址（通过当前进程的 trap_cx_ppn）
        // 调用 current_trap_cx_kernel_addr()
        "call current_trap_cx_kernel_addr",
        // a0 = TrapContext 内核地址

        // 从 TrapContext 加载用户 satp（存在 kernel_satp 字段的下一个字段）
        // 实际上我们需要从 user_satp 字段读取
        // TrapContext: x[32], sstatus, sepc, kernel_sp, kernel_satp, trap_handler, user_satp
        // offsets: 256, 264, 272, 280, 288, 296
        "ld t1, 296(a0)",  // user_satp

        // 计算 __restore 的虚拟地址
        "la t0, strampoline",
        "la t2, __restore",
        "sub t2, t2, t0",
        "li t0, 0x3ffff000",  // TRAMPOLINE
        "add t2, t0, t2",

        // 切换到用户页表并跳转到 __restore
        // __restore(a0 = TrapContext内核地址, a1 = user_satp)
        // 但 __restore 是在 trampoline 中的，需要在用户页表下执行
        "mv a0, a0",
        "jr t2",  // 跳转到 __restore (trampoline 中的版本)
    );
}

/// 获取当前进程的 TrapContext 内核虚拟地址
#[no_mangle]
pub extern "C" fn current_trap_cx_kernel_addr() -> usize {
    let task = crate::task::current_task().expect("no current task");
    let inner = task.inner_exclusive_access();
    let pa: crate::mm::PhysAddr = inner.trap_cx_ppn.into();
    // 在内核恒等映射下，物理地址 = 内核虚拟地址
    pa.0
}
