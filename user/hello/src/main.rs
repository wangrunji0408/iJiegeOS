#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

const SYS_WRITE: usize = 64;
const SYS_EXIT: usize = 93;

unsafe fn syscall3(n: usize, a0: usize, a1: usize, a2: usize) -> usize {
    let r;
    asm!(
        "ecall",
        in("a7") n,
        inlateout("a0") a0 => r,
        in("a1") a1,
        in("a2") a2,
    );
    r
}

fn write(fd: usize, buf: &[u8]) {
    unsafe { syscall3(SYS_WRITE, fd, buf.as_ptr() as usize, buf.len()); }
}

fn exit(code: i32) -> ! {
    unsafe { syscall3(SYS_EXIT, code as usize, 0, 0); }
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    write(1, b"hello from user!\n");
    exit(0);
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! { exit(1); }
