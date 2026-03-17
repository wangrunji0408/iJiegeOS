use alloc::sync::Arc;
use core::arch::asm;
use crate::task::{Task, TaskState, TaskContext, CURRENT_TASK, TASK_MANAGER};

/// 切换上下文汇编
#[unsafe(naked)]
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
            log::error!("No more tasks! system idle");
            loop {
                crate::arch::wait_for_interrupt();
            }
        }
    }
}

/// trap_return: 从内核态返回用户态
/// 当新任务第一次被调度时，ra = trap_return
/// 此时 sp 指向内核栈中的 TrapContext
/// 我们需要：
///   1. 设置 sscratch = sp（让 __alltraps 知道内核栈在哪）
///   2. 跳转到 __restore 完成返回用户态
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn trap_return() {
    core::arch::naked_asm!(
        // sp 当前指向 TrapContext（内核栈顶的 TrapContext 起始位置）
        // 设置 sscratch = sp，这样下次陷阱时 __alltraps 可以正确切换
        "csrw sscratch, sp",
        // 跳转到 __restore 来恢复用户态寄存器并执行 sret
        "j __restore",
    );
}
