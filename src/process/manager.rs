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
    idle_task_cx: TaskContext,
}

lazy_static! {
    static ref MANAGER: Mutex<ProcessManager> = Mutex::new(ProcessManager {
        current: None,
        processes: Vec::new(),
        ready_queue: VecDeque::new(),
        idle_task_cx: TaskContext::default(),
    });
}

pub fn init() {
    // Nothing to do initially, processes are created via exec
}

pub fn add_process(proc: Arc<Mutex<Process>>) -> usize {
    let mut mgr = MANAGER.lock();
    let pid = proc.lock().pid;

    // Ensure processes vec is large enough
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
    let mgr = MANAGER.lock();
    mgr.current.unwrap_or(0)
}

pub fn get_process(pid: usize) -> Option<Arc<Mutex<Process>>> {
    let mgr = MANAGER.lock();
    mgr.processes.get(pid).and_then(|p| p.clone())
}

pub fn run_first_task() -> ! {
    // Load the init process (should have been created by fs::init or similar)
    let mut mgr = MANAGER.lock();
    if mgr.ready_queue.is_empty() {
        drop(mgr);
        // Try to create init process from embedded binary
        crate::fs::load_init_process();
        mgr = MANAGER.lock();
    }

    if let Some(pid) = mgr.ready_queue.pop_front() {
        mgr.current = Some(pid);
        let proc = mgr.processes[pid].as_ref().unwrap().clone();
        let mut p = proc.lock();
        p.state = ProcessState::Running;

        // Set up trap return
        let trap_cx = &p.trap_cx as *const _ as usize;
        let kernel_satp = p.trap_cx.kernel_satp;
        let kernel_sp = p.trap_cx.kernel_sp;
        let user_satp = p.token();
        let task_cx_ptr = &p.task_cx as *const TaskContext;

        drop(p);
        drop(mgr);

        // Set sstatus and jump to user mode via trap return
        extern "C" {
            fn __restore();
        }

        // Set satp to user space page table
        unsafe {
            // Set up sscratch with the kernel stack pointer for trap entry
            let proc_ref = proc.lock();
            let sp = proc_ref.trap_cx.kernel_sp;
            let sepc = proc_ref.trap_cx.sepc;
            let sstatus = proc_ref.trap_cx.sstatus;
            let user_sp = proc_ref.trap_cx.x[2];
            drop(proc_ref);

            // Push trap context onto kernel stack and restore from it
            // We need to set up a proper trap frame for __restore to work
            setup_first_return(proc, pid);
        }
    } else {
        panic!("No tasks to run!");
    }
    unreachable!()
}

unsafe fn setup_first_return(proc: Arc<Mutex<Process>>, pid: usize) -> ! {
    let p = proc.lock();
    let satp = p.token();
    let trap_cx = p.trap_cx.clone();
    let kernel_sp = p.kernel_stack + crate::config::KERNEL_STACK_SIZE;
    drop(p);

    // Set satp to user page table
    riscv::register::satp::write(satp);
    core::arch::asm!("sfence.vma");

    // Set sscratch to user stack pointer
    let user_sp = trap_cx.x[2];

    // Set up the trap frame on the kernel stack
    let sp = kernel_sp - core::mem::size_of::<crate::trap::TrapContext>();
    let cx = &mut *(sp as *mut crate::trap::TrapContext);
    *cx = trap_cx;

    // Set sstatus and sepc
    core::arch::asm!("csrw sstatus, {}", in(reg) cx.sstatus);
    core::arch::asm!("csrw sepc, {}", in(reg) cx.sepc);
    core::arch::asm!("csrw sscratch, {}", in(reg) cx.x[2]);

    // Restore all registers and sret
    core::arch::asm!(
        "mv sp, {sp}",
        // Restore registers
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
        // Restore sp from sscratch
        "addi sp, sp, 34*8",
        "csrrw sp, sscratch, sp",
        "sret",
        sp = in(reg) sp,
        options(noreturn),
    )
}

pub fn yield_current() {
    // Simple round-robin: put current back on queue and pick next
    let mut mgr = MANAGER.lock();
    if let Some(pid) = mgr.current {
        if let Some(proc) = &mgr.processes[pid] {
            let mut p = proc.lock();
            if p.state == ProcessState::Running {
                p.state = ProcessState::Ready;
                mgr.ready_queue.push_back(pid);
            }
        }
    }

    if let Some(next_pid) = mgr.ready_queue.pop_front() {
        let old_pid = mgr.current.replace(next_pid);
        if old_pid == Some(next_pid) {
            // Same process, do nothing
            if let Some(proc) = &mgr.processes[next_pid] {
                proc.lock().state = ProcessState::Running;
            }
            return;
        }
        if let Some(proc) = &mgr.processes[next_pid] {
            proc.lock().state = ProcessState::Running;
        }

        // Get task context pointers
        let old_cx: *mut TaskContext;
        let new_cx: *const TaskContext;

        if let Some(old) = old_pid {
            if let Some(proc) = &mgr.processes[old] {
                old_cx = &mut proc.lock().task_cx as *mut TaskContext;
            } else {
                old_cx = &mut mgr.idle_task_cx as *mut TaskContext;
            }
        } else {
            old_cx = &mut mgr.idle_task_cx as *mut TaskContext;
        }

        if let Some(proc) = &mgr.processes[next_pid] {
            new_cx = &proc.lock().task_cx as *const TaskContext;
        } else {
            return;
        }

        drop(mgr);
        unsafe {
            __switch(old_cx, new_cx);
        }
    } else {
        // No other tasks, continue running current
        if let Some(pid) = mgr.current {
            if let Some(proc) = &mgr.processes[pid] {
                proc.lock().state = ProcessState::Running;
            }
        }
    }
}

pub fn exit_current(code: i32) -> ! {
    let mut mgr = MANAGER.lock();
    if let Some(pid) = mgr.current.take() {
        if let Some(proc) = &mgr.processes[pid] {
            let mut p = proc.lock();
            p.state = ProcessState::Zombie;
            p.exit_code = code;
            println!("[kernel] Process {} exited with code {}", pid, code);
        }
    }

    // Try to run next task
    if let Some(next_pid) = mgr.ready_queue.pop_front() {
        mgr.current = Some(next_pid);
        if let Some(proc) = &mgr.processes[next_pid] {
            proc.lock().state = ProcessState::Running;
            let cx_ptr = &proc.lock().task_cx as *const TaskContext;
            drop(mgr);
            unsafe {
                let mut dummy = TaskContext::default();
                __switch(&mut dummy as *mut TaskContext, cx_ptr);
            }
        }
    }
    drop(mgr);
    println!("[kernel] All processes exited, shutting down.");
    crate::arch::shutdown();
}

pub fn do_fork(parent_pid: usize) -> isize {
    // TODO: implement fork
    -1
}

pub fn do_wait(pid: isize) -> (isize, i32) {
    // TODO: implement wait
    (-1, 0)
}
