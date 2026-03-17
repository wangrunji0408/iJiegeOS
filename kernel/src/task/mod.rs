mod task;
mod manager;
mod scheduler;
mod pid;
mod context;

pub use task::{Task, TaskState, TaskInner};
pub use manager::{add_task, remove_task, get_task, TASK_MANAGER};
pub use scheduler::{run_first_task, suspend_current_and_run_next, exit_current_and_run_next};
pub use context::TaskContext;
pub use pid::{Pid, PidAllocator};

use alloc::sync::Arc;
use spin::Mutex;
use crate::arch::trap::TrapContext;
use crate::signal::Signal;
use lazy_static::lazy_static;

lazy_static! {
    /// 当前 CPU 上运行的任务
    pub static ref CURRENT_TASK: Mutex<Option<Arc<Task>>> = Mutex::new(None);
}

pub fn init() {
    manager::init();
    // 加载初始进程
    loader::load_init_proc();
    log::info!("task: init process loaded");
}

mod loader {
    pub fn load_init_proc() {
        let init_elf = include_bytes!("../../initrd/init");
        let task = super::Task::new_from_elf(init_elf);
        super::add_task(alloc::sync::Arc::new(task));
    }
}

/// 获取当前任务
pub fn current_task() -> Option<Arc<Task>> {
    CURRENT_TASK.lock().clone()
}

/// 获取当前任务的 TrapContext
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().unwrap().inner_exclusive_access().get_trap_cx()
}

/// 获取当前任务的用户页表 token
pub fn current_user_token() -> usize {
    current_task().unwrap().inner_exclusive_access().get_user_token()
}

/// 给当前任务添加信号
pub fn current_add_signal(signal: Signal) {
    if let Some(task) = current_task() {
        task.inner_exclusive_access().pending_signals.push(signal);
    }
}

/// 处理当前任务的待处理信号
pub fn handle_signals() {
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        let signals: alloc::vec::Vec<Signal> = inner.pending_signals.drain(..).collect();
        drop(inner);

        for sig in signals {
            match sig {
                Signal::SIGSEGV | Signal::SIGKILL | Signal::SIGABRT => {
                    log::warn!("Process killed by signal {:?}", sig);
                    drop(task.inner_exclusive_access().pending_signals.drain(..));
                    exit_current_and_run_next(-(sig as i32) as usize);
                    return;
                }
                Signal::SIGCHLD => {
                    // 通知父进程
                    // TODO: 唤醒等待的父进程
                }
                _ => {
                    // 默认处理
                }
            }
        }
    }
}
