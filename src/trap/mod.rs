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
            println!(
                "[kernel] regs: ra={:#x} sp={:#x} gp={:#x} tp={:#x}",
                cx.x[1], cx.x[2], cx.x[3], cx.x[4]
            );
            println!(
                "[kernel] a0={:#x} a1={:#x} a2={:#x} a3={:#x} a4={:#x} a5={:#x}",
                cx.x[10], cx.x[11], cx.x[12], cx.x[13], cx.x[14], cx.x[15]
            );
            println!(
                "[kernel] s0={:#x} s1={:#x} s2={:#x} s3={:#x}",
                cx.x[8], cx.x[9], cx.x[18], cx.x[19]
            );
            // Print stack backtrace (follow ra/fp chain)
            println!("[kernel] Stack backtrace:");
            let mut fp = cx.x[8]; // s0/fp
            let mut ra_val = cx.x[1];
            for i in 0..5 {
                println!("[kernel]   #{}: ra={:#x}", i, ra_val);
                if fp == 0 || fp < 0x1000 { break; }
                // Try to read saved ra and fp from stack frame
                let proc_arc = crate::process::current_process();
                let proc_guard = proc_arc.lock();
                let pt = &proc_guard.memory_set.page_table;
                if let Some(pte) = pt.translate(crate::mm::VirtPageNum(fp / 4096)) {
                    let pa = pte.ppn().addr().0 + (fp & 4095);
                    if (fp & 4095) >= 16 {
                        unsafe {
                            ra_val = *((pa - 8) as *const usize);
                            fp = *((pa - 16) as *const usize);
                        }
                    } else { break; }
                } else { break; }
                drop(proc_guard);
            }
            // Try to handle page fault
            let handled = crate::process::handle_page_fault(stval, scause.cause());
            if !handled {
                println!("[kernel] Unhandled page fault at {:#x}, addr={:#x}", cx.sepc, stval);
                // Print last syscalls
                unsafe {
                    let idx = crate::syscall::SC_IDX;
                    println!("[kernel] Last syscalls:");
                    for i in 0..8 {
                        let j = (idx + i) % 8;
                        let (id, ret) = crate::syscall::LAST_SYSCALLS[j];
                        if id != 0 {
                            println!("[kernel]   syscall {} -> {}", id, ret);
                        }
                    }
                }
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
