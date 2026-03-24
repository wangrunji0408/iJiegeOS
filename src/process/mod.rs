mod pid;
mod process;
mod manager;
mod switch;

pub use process::*;
pub use manager::*;
pub use pid::*;

use alloc::sync::Arc;
use spin::Mutex;

pub fn init() {
    manager::init();
}

pub fn run_first_task() -> ! {
    manager::run_first_task()
}

pub fn exit_current(code: i32) -> ! {
    manager::exit_current(code)
}

pub fn schedule() {
    manager::yield_current();
}

pub fn handle_page_fault(addr: usize, cause: riscv::register::scause::Trap) -> bool {
    // For now, no page fault handling
    false
}

pub fn current_process() -> Arc<Mutex<Process>> {
    manager::current_process()
}

pub fn current_pid() -> usize {
    manager::current_pid()
}
