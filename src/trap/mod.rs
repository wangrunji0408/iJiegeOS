mod context;

pub use context::TrapContext;

use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    stval, stvec, sepc, sie,
};

core::arch::global_asm!(include_str!("trap.S"));

pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        stvec::write(__alltraps as usize, stvec::TrapMode::Direct);
        // Enable timer interrupt
        sie::set_stimer();
        sie::set_sext();
    }
}

pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

#[no_mangle]
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            let result = crate::syscall::syscall(
                cx.x[17], // a7 = syscall number
                [cx.x[10], cx.x[11], cx.x[12], cx.x[13], cx.x[14], cx.x[15]], // a0-a5
                cx,
            );
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            println!(
                "[kernel] PageFault at {:#x}, bad addr = {:#x}, scause = {:?}",
                cx.sepc, stval, scause.cause()
            );
            // Try to handle page fault
            let handled = crate::process::handle_page_fault(stval, scause.cause());
            if !handled {
                println!("[kernel] Unhandled page fault, killing process");
                crate::process::exit_current(-2);
            }
        }
        Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault) => {
            println!(
                "[kernel] InstructionPageFault at {:#x}, bad addr = {:#x}",
                cx.sepc, stval
            );
            crate::process::exit_current(-2);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!(
                "[kernel] IllegalInstruction at {:#x}, instruction = {:#x}",
                cx.sepc, stval
            );
            crate::process::exit_current(-2);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            crate::arch::sbi::set_timer(get_time() + crate::config::CLOCK_FREQ as u64 / 100);
            crate::process::schedule();
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            // Handle external interrupts (e.g., virtio)
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}, sepc = {:#x}",
                scause.cause(),
                stval,
                cx.sepc,
            );
        }
    }
    cx
}

fn get_time() -> u64 {
    riscv::register::time::read() as u64
}

pub fn set_next_trigger() {
    crate::arch::sbi::set_timer(get_time() + crate::config::CLOCK_FREQ as u64 / 100);
}
