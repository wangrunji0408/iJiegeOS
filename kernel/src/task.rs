//! Task management (placeholder until full impl lands)
use core::sync::atomic::{AtomicUsize, Ordering};

static CURRENT_PID: AtomicUsize = AtomicUsize::new(0);

pub fn init() {}
pub fn current_pid_opt() -> Option<usize> { None }
pub fn scheduler_tick() {}
pub fn handle_page_fault(_stval: usize) -> bool { false }
pub fn exit_current(_code: i32) -> ! {
    crate::sbi::shutdown(true);
}
