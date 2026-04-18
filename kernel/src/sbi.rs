use core::arch::asm;

const SBI_CONSOLE_PUTCHAR: usize = 1;
const SBI_SHUTDOWN: usize = 8;

/// Legacy SBI console putchar (works on OpenSBI in all SBI versions)
#[inline]
pub fn console_putchar(c: usize) {
    unsafe {
        asm!(
            "ecall",
            in("a7") SBI_CONSOLE_PUTCHAR,
            in("a0") c,
            lateout("a0") _,
        );
    }
}

pub fn shutdown(failure: bool) -> ! {
    // Try SRST extension first
    let _ = sbi_call(0x53525354, 0, if failure { 1 } else { 0 }, 0, 0);
    // Fallback to legacy shutdown
    unsafe {
        asm!("ecall", in("a7") SBI_SHUTDOWN, options(noreturn))
    }
}

#[allow(dead_code)]
fn sbi_call(eid: usize, fid: usize, a0: usize, a1: usize, a2: usize) -> (usize, usize) {
    let (err, val);
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") a0 => err,
            inlateout("a1") a1 => val,
            in("a2") a2,
            in("a6") fid,
            in("a7") eid,
        );
    }
    (err, val)
}
