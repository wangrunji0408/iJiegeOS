//! Trap handling (S-mode).

use core::arch::global_asm;
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
    stvec::TrapMode,
};

pub mod context;
pub use context::{trap_return, TrapContext};

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

pub fn enable_external_interrupt() {
    unsafe { sie::set_sext(); }
}

#[no_mangle]
pub extern "C" fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            let ret = crate::syscall::syscall(
                cx.x[17],
                [cx.x[10], cx.x[11], cx.x[12], cx.x[13], cx.x[14], cx.x[15]],
                cx,
            );
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
            if crate::task::handle_page_fault(stval) { return cx; }
            crate::println!(
                "[kernel] PF cause={:?} stval={:#x} sepc={:#x} ra={:#x} sp={:#x}",
                scause.cause(), stval, cx.sepc, cx.x[1], cx.x[2]
            );
            crate::task::exit_current(-11);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            crate::println!("[kernel] illegal instr sepc={:#x}", cx.sepc);
            crate::task::exit_current(-1);
        }
        _ => {
            panic!(
                "unhandled trap {:?}, stval={:#x}, sepc={:#x}",
                scause.cause(), stval, cx.sepc
            );
        }
    }
    cx
}
