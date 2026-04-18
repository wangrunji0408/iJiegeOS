#![no_std]
#![no_main]
#![feature(naked_functions)]
#![allow(dead_code)]

extern crate alloc;

use core::arch::global_asm;
use core::panic::PanicInfo;

#[macro_use]
mod console;
mod sbi;

global_asm!(include_str!("entry.S"));

#[no_mangle]
pub extern "C" fn rust_main(dtb: usize) -> ! {
    println!();
    println!("[kernel] hello from rust, dtb @ {:#x}", dtb);
    println!("[kernel] shutting down");
    sbi::shutdown(false);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[kernel] PANIC: {}", info);
    sbi::shutdown(true);
}
