use core::arch::global_asm;

global_asm!(include_str!("boot/entry.asm"));

pub mod trap;
pub mod mm;
pub mod sbi;

pub fn init() {
    trap::init();
}

pub fn wait_for_interrupt() {
    unsafe {
        core::arch::asm!("wfi");
    }
}
