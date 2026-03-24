/// SBI call wrapper
#[inline(always)]
fn sbi_call(eid: usize, fid: usize, arg0: usize, arg1: usize, arg2: usize) -> (usize, usize) {
    let error: usize;
    let value: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") arg0 => error,
            inlateout("a1") arg1 => value,
            in("a2") arg2,
            in("a6") fid,
            in("a7") eid,
        );
    }
    (error, value)
}

/// Console putchar (legacy)
pub fn console_putchar(c: u8) {
    sbi_call(0x01, 0, c as usize, 0, 0);
}

/// Console getchar (legacy)
pub fn console_getchar() -> Option<u8> {
    let (_, value) = sbi_call(0x02, 0, 0, 0, 0);
    if value == usize::MAX {
        None
    } else {
        Some(value as u8)
    }
}

/// Set timer
pub fn set_timer(stime_value: u64) {
    sbi_call(0x54494D45, 0, stime_value as usize, 0, 0);
}

/// Shutdown
pub fn shutdown() -> ! {
    sbi_call(0x53525354, 0, 0, 0, 0);
    unreachable!()
}
