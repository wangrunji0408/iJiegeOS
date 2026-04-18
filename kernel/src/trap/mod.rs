//! Trap handling (S-mode) for both user and kernel traps.

use core::arch::global_asm;
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sie, sstatus, stval, stvec,
    stvec::TrapMode,
};

pub mod context;
pub use context::TrapContext;

global_asm!(include_str!("trap.S"));

extern "C" {
    fn __alltraps();
    fn __restore(cx_addr: usize);
}

pub fn init() {
    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

pub fn enable_timer_interrupt() {
    unsafe { sie::set_stimer(); }
}

#[no_mangle]
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            let ret = crate::syscall::syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12], cx.x[13], cx.x[14], cx.x[15]], cx);
            cx.x[10] = ret as usize;
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            crate::timer::tick();
            crate::task::scheduler_tick();
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            crate::plic::handle_external();
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::InstructionFault) => {
            let pid = crate::task::current_pid_opt().unwrap_or(usize::MAX);
            crate::println!(
                "[kernel] PF pid={} cause={:?} stval={:#x} sepc={:#x}",
                pid, scause.cause(), stval, cx.sepc
            );
            // Attempt lazy-fill for mmap/brk
            if crate::task::handle_page_fault(stval) {
                return cx;
            }
            crate::println!("[kernel] unhandled page fault, killing task");
            crate::task::exit_current(-11);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            crate::println!("[kernel] illegal instruction sepc={:#x}", cx.sepc);
            crate::task::exit_current(-1);
        }
        _ => {
            panic!("unhandled trap {:?}, stval={:#x}, sepc={:#x}", scause.cause(), stval, cx.sepc);
        }
    }
    cx
}

pub use crate::trap::context::trap_return;

#[allow(dead_code)]
pub fn set_kernel_trap() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

#[no_mangle]
extern "C" fn trap_from_kernel() -> ! {
    let scause = scause::read();
    let stval = stval::read();
    panic!("trap from kernel: {:?} stval={:#x}", scause.cause(), stval);
}
