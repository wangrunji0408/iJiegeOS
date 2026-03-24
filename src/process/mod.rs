pub fn init() {
    // TODO: Initialize process management
}

pub fn run_first_task() -> ! {
    panic!("No tasks to run yet");
}

pub fn exit_current(code: i32) -> ! {
    println!("[kernel] Process exit with code {}", code);
    crate::arch::shutdown();
}

pub fn schedule() {
    // TODO: Schedule next task
}

pub fn handle_page_fault(_addr: usize, _cause: riscv::register::scause::Trap) -> bool {
    false
}
