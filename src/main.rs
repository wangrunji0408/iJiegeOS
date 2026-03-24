#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![allow(unused)]

extern crate alloc;

#[macro_use]
mod console;
mod arch;
mod config;
mod drivers;
mod fs;
mod mm;
mod net;
mod process;
mod syscall;
mod trap;

use core::arch::global_asm;

global_asm!(include_str!("arch/riscv64/entry.S"));

/// Clear BSS segment
fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

#[no_mangle]
pub fn rust_main(hartid: usize, dtb: usize) -> ! {
    clear_bss();
    console::init();
    println!("[JiegeOS] Booting on hart {}", hartid);
    println!("[JiegeOS] DTB at {:#x}", dtb);

    mm::init();
    println!("[JiegeOS] Memory management initialized");

    trap::init();
    println!("[JiegeOS] Trap handler initialized");

    drivers::init(dtb);
    println!("[JiegeOS] Drivers initialized");

    fs::init();
    println!("[JiegeOS] File system initialized");

    process::init();
    println!("[JiegeOS] Process management initialized");

    net::init();
    println!("[JiegeOS] Network initialized");

    // Pre-poll network to handle initial ARP
    for i in 0..100 {
        net::poll_net();
        for _ in 0..10000 { core::hint::spin_loop(); }
    }
    println!("[JiegeOS] Network pre-polled");

    process::run_first_task();
    unreachable!("Should not reach here");
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "\x1b[1;31m[PANIC] at {}:{} {}\x1b[0m",
            location.file(),
            location.line(),
            info.message()
        );
    } else {
        println!("\x1b[1;31m[PANIC] {}\x1b[0m", info.message());
    }
    arch::shutdown();
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Alloc error: {:?}", layout);
}
