use core::arch::global_asm;

global_asm!(include_str!("boot/entry.asm"));

pub mod trap;
pub mod sbi;
pub mod mm;

pub use trap::TrapContext;

pub fn init() {
    trap::init();
}

pub fn wait_for_interrupt() {
    unsafe {
        core::arch::asm!("wfi");
    }
}
