/// 信号系统调用

use crate::arch::trap::TrapContext;
use crate::task::current_task;
use super::errno::*;

pub fn sys_rt_sigaction(signum: i32, act: *const u8, oldact: *mut u8, sigsetsize: usize) -> i64 {
    // 简化：忽略信号处理器注册
    // 返回成功即可
    0
}

pub fn sys_rt_sigprocmask(how: i32, set: *const u64, oldset: *mut u64, sigsetsize: usize) -> i64 {
    // 简化：忽略信号掩码
    if !oldset.is_null() {
        let tok = crate::task::current_user_token();
        *crate::mm::translated_refmut(tok, oldset) = 0u64;
    }
    0
}

pub fn sys_rt_sigreturn(ctx: &mut TrapContext) -> i64 {
    0
}

pub fn sys_kill(pid: i32, sig: i32) -> i64 {
    if sig == 0 { return 0; }  // kill -0 用于检查进程是否存在

    if let Some(signal) = crate::signal::Signal::from_num(sig as u32) {
        if pid > 0 {
            if let Some(task) = crate::task::get_task(pid) {
                task.inner_exclusive_access().pending_signals.push(signal);
                return 0;
            }
        } else if pid == 0 || pid == -1 {
            // 发送给所有进程
            if let Some(task) = current_task() {
                task.inner_exclusive_access().pending_signals.push(signal);
            }
            return 0;
        }
    }
    ESRCH
}

pub fn sys_tkill(pid: i32, tid: i32, sig: i32) -> i64 {
    sys_kill(pid, sig)
}

pub fn sys_rt_sigsuspend(set: *const u64, sigsetsize: usize) -> i64 {
    // 简化：让出 CPU
    crate::task::suspend_current_and_run_next();
    EINTR
}
