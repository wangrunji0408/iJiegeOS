/// SBI (Supervisor Binary Interface) 调用接口
/// 通过 ecall 调用 OpenSBI

#[allow(unused)]
mod sbi_call {
    const SBI_SET_TIMER: usize = 0;
    const SBI_CONSOLE_PUTCHAR: usize = 1;
    const SBI_CONSOLE_GETCHAR: usize = 2;
    const SBI_CLEAR_IPI: usize = 3;
    const SBI_SEND_IPI: usize = 4;
    const SBI_REMOTE_FENCE_I: usize = 5;
    const SBI_REMOTE_SFENCE_VMA: usize = 6;
    const SBI_SHUTDOWN: usize = 8;

    // SBI extension IDs (new-style)
    pub const EXT_BASE: usize = 0x10;
    pub const EXT_TIMER: usize = 0x54494D45;
    pub const EXT_SRST: usize = 0x53525354;
    pub const EXT_DBCN: usize = 0x4442434E;

    #[inline(always)]
    pub fn sbi_call_legacy(which: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
        let ret;
        unsafe {
            core::arch::asm!(
                "ecall",
                inlateout("a0") arg0 => ret,
                in("a1") arg1,
                in("a2") arg2,
                in("a7") which,
            );
        }
        ret
    }

    #[inline(always)]
    pub fn sbi_call(eid: usize, fid: usize, arg0: usize, arg1: usize, arg2: usize) -> (usize, usize) {
        let (error, value);
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
}

pub fn console_putchar(c: u8) {
    sbi_call::sbi_call_legacy(1, c as usize, 0, 0);
}

pub fn console_getchar() -> u8 {
    sbi_call::sbi_call_legacy(2, 0, 0, 0) as u8
}

pub fn set_timer(time: u64) {
    sbi_call::sbi_call(sbi_call::EXT_TIMER, 0, time as usize, 0, 0);
}

pub fn shutdown() -> ! {
    sbi_call::sbi_call(sbi_call::EXT_SRST, 0, 0, 0, 0);
    unreachable!()
}

/// 获取 strampoline 函数的物理地址
pub fn strampoline_addr() -> usize {
    extern "C" {
        fn strampoline();
    }
    strampoline as usize
}
