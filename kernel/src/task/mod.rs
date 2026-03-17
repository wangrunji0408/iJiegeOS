pub mod task;
pub mod manager;
pub mod scheduler;
pub mod pid;
pub mod context;

pub use task::{Task, TaskState, TaskInner, KernelStack};
pub use manager::{add_task, remove_task, get_task, TASK_MANAGER};
pub use scheduler::{run_first_task, suspend_current_and_run_next, exit_current_and_run_next};
pub use context::TaskContext;
pub use pid::{Pid, PidAllocator, PID_ALLOCATOR};

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
    use alloc::vec::Vec;

    pub fn load_init_proc() {
        // 从文件系统读取 /init 或 /usr/sbin/nginx
        // 这需要在 fs::init() 之后调用
        let paths = ["/init", "/usr/sbin/nginx", "/bin/sh"];
        for path in &paths {
            if let Some(elf_data) = read_file(path) {
                crate::println!("Loading init from: {}", path);
                let task = super::Task::new_from_elf_with_args(&elf_data, path, &[], &[]);
                super::add_task(alloc::sync::Arc::new(task));
                return;
            }
        }
        panic!("No init binary found! Checked: {:?}", paths);
    }

    fn read_file(path: &str) -> Option<Vec<u8>> {
        use crate::fs;
        let file = fs::open(path, 0, 0)?;  // O_RDONLY
        let stat = file.stat();
        let size = stat.st_size as usize;
        if size == 0 { return None; }
        let mut buf = alloc::vec![0u8; size];
        let mut offset = 0;
        while offset < size {
            let n = file.read(&mut buf[offset..]);
            if n <= 0 { break; }
            offset += n as usize;
        }
        if offset > 0 {
            buf.truncate(offset);
            Some(buf)
        } else {
            None
        }
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
