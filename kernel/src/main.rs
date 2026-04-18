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
mod initramfs;
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
mod virtio_hal;

global_asm!(include_str!("entry.S"));

static HTTPD_ELF: &[u8] = include_bytes!("../../user/httpd/target/riscv64gc-unknown-none-elf/release/httpd");
static LD_MUSL_ELF: &[u8] = include_bytes!("../../vendor/musl/lib/ld-musl-riscv64.so.1");
static NGINX_ELF: &[u8] = include_bytes!("../../vendor/nginx/usr/sbin/nginx");

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
    println!("[kernel] subsystems up, launching nginx via ld-musl");

    let t = task::Task::from_program(
        LD_MUSL_ELF,
        None,
        &["/lib/ld-musl-riscv64.so.1", "/usr/sbin/nginx", "-c", "/etc/nginx/nginx.conf"],
        &[
            "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
            "HOME=/root",
            "LD_LIBRARY_PATH=/lib:/usr/lib",
            "TERM=xterm",
        ],
    );
    task::add_task(t);
    trap::enable_timer_interrupt();
    task::run_next();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[kernel] PANIC: {}", info);
    sbi::shutdown(true);
}
