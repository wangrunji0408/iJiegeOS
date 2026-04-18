#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![allow(dead_code)]
#![allow(static_mut_refs)]
#![allow(unused_variables)]
#![allow(unused_imports)]

extern crate alloc;

use core::arch::global_asm;
use core::panic::PanicInfo;

#[macro_use]
mod console;
mod fs;
mod heap;
mod loader;
mod mm;
mod net;
mod plic;
mod sbi;
mod syscall;
mod syscall_impl;
mod task;
mod timer;
mod trap;

global_asm!(include_str!("entry.S"));

static HELLO_ELF: &[u8] = include_bytes!("../../user/hello/target/riscv64gc-unknown-none-elf/release/hello");

#[no_mangle]
pub extern "C" fn rust_main(dtb: usize) -> ! {
    heap::init();
    println!();
    println!("[kernel] boot, dtb @ {:#x}", dtb);
    mm::init();
    trap::init();
    timer::init();
    plic::init();
    net::init();
    fs::init();
    task::init();
    trap::enable_timer_interrupt();
    println!("[kernel] subsystems up, launching hello");

    let t = task::Task::from_elf(HELLO_ELF, &["hello"], &["PATH=/bin"]);
    task::add_task(t);
    task::run_next();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[kernel] PANIC: {}", info);
    sbi::shutdown(true);
}
