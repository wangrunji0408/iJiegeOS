use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use lazy_static::lazy_static;
use super::process::{Process, ProcessState};
use super::switch::{TaskContext, __switch};

struct ProcessManager {
    current: Option<usize>,
    processes: Vec<Option<Arc<Mutex<Process>>>>,
    ready_queue: VecDeque<usize>,
}

lazy_static! {
    static ref MANAGER: Mutex<ProcessManager> = Mutex::new(ProcessManager {
        current: None,
        processes: Vec::new(),
        ready_queue: VecDeque::new(),
    });
}

// Idle task context for when there's no process to switch from
static mut IDLE_TASK_CX: TaskContext = TaskContext { ra: 0, sp: 0, s: [0; 12] };

pub fn init() {}

pub fn add_process(proc: Arc<Mutex<Process>>) -> usize {
    let mut mgr = MANAGER.lock();
    let pid = proc.lock().pid;
    while mgr.processes.len() <= pid {
        mgr.processes.push(None);
    }
    mgr.processes[pid] = Some(proc);
    mgr.ready_queue.push_back(pid);
    pid
}

pub fn current_process() -> Arc<Mutex<Process>> {
    let mgr = MANAGER.lock();
    let pid = mgr.current.expect("No current process");
    mgr.processes[pid].as_ref().unwrap().clone()
}

pub fn current_pid() -> usize {
    MANAGER.lock().current.unwrap_or(0)
}

pub fn get_process(pid: usize) -> Option<Arc<Mutex<Process>>> {
    let mgr = MANAGER.lock();
    mgr.processes.get(pid).and_then(|p| p.clone())
}

pub fn run_first_task() -> ! {
    {
        let mgr = MANAGER.lock();
        if mgr.ready_queue.is_empty() {
            drop(mgr);
            crate::fs::load_init_process();
        }
    }

    let mut mgr = MANAGER.lock();
    if let Some(pid) = mgr.ready_queue.pop_front() {
        mgr.current = Some(pid);
        let proc = mgr.processes[pid].as_ref().unwrap().clone();
        proc.lock().state = ProcessState::Running;
        drop(mgr);

        // Jump to the process by calling trap_return
        trap_return_first(proc);
    } else {
        panic!("No tasks to run!");
    }
}

fn trap_return_first(proc: Arc<Mutex<Process>>) -> ! {
    let p = proc.lock();
    let satp = p.token();
    let trap_cx = p.trap_cx.clone();
    let kernel_sp = p.kernel_stack + crate::config::KERNEL_STACK_SIZE;
    drop(p);

    unsafe {
        // Switch to user page table
        riscv::register::satp::write(satp);
        core::arch::asm!("sfence.vma");

        // Set up trap frame on kernel stack
        let sp = kernel_sp - core::mem::size_of::<crate::trap::TrapContext>();
        let cx_ptr = sp as *mut crate::trap::TrapContext;
        *cx_ptr = trap_cx;

        // Set CSRs
        core::arch::asm!("csrw sstatus, {}", in(reg) (*cx_ptr).sstatus);
        core::arch::asm!("csrw sepc, {}", in(reg) (*cx_ptr).sepc);
        core::arch::asm!("csrw sscratch, {}", in(reg) (*cx_ptr).x[2]);

        // Restore registers and sret
        core::arch::asm!(
            "mv sp, {sp}",
            "ld x1, 1*8(sp)",
            "ld x3, 3*8(sp)",
            "ld x5, 5*8(sp)",
            "ld x6, 6*8(sp)",
            "ld x7, 7*8(sp)",
            "ld x8, 8*8(sp)",
            "ld x9, 9*8(sp)",
            "ld x10, 10*8(sp)",
            "ld x11, 11*8(sp)",
            "ld x12, 12*8(sp)",
            "ld x13, 13*8(sp)",
            "ld x14, 14*8(sp)",
            "ld x15, 15*8(sp)",
            "ld x16, 16*8(sp)",
            "ld x17, 17*8(sp)",
            "ld x18, 18*8(sp)",
            "ld x19, 19*8(sp)",
            "ld x20, 20*8(sp)",
            "ld x21, 21*8(sp)",
            "ld x22, 22*8(sp)",
            "ld x23, 23*8(sp)",
            "ld x24, 24*8(sp)",
            "ld x25, 25*8(sp)",
            "ld x26, 26*8(sp)",
            "ld x27, 27*8(sp)",
            "ld x28, 28*8(sp)",
            "ld x29, 29*8(sp)",
            "ld x30, 30*8(sp)",
            "ld x31, 31*8(sp)",
            "addi sp, sp, 34*8",
            "csrrw sp, sscratch, sp",
            "sret",
            sp = in(reg) sp,
            options(noreturn),
        )
    }
}

pub fn yield_current() {
    // For now with single process, just return
    let mgr = MANAGER.lock();
    if mgr.ready_queue.is_empty() {
        return;
    }
    drop(mgr);
    // TODO: implement actual context switch
}

pub fn exit_current(code: i32) -> ! {
    {
        let mgr = MANAGER.lock();
        if let Some(pid) = mgr.current {
            if let Some(proc) = &mgr.processes[pid] {
                let mut p = proc.lock();
                p.state = ProcessState::Zombie;
                p.exit_code = code;
                println!("[kernel] Process {} exited with code {}", pid, code);
            }
        }
    }
    println!("[kernel] All processes exited, shutting down.");
    crate::arch::shutdown();
}

pub fn do_fork(_parent_pid: usize) -> isize {
    -1 // TODO
}

pub fn do_wait(_pid: isize) -> (isize, i32) {
    (-1, 0) // TODO
}
