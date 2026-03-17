#[cfg(target_arch = "riscv64")]
mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

pub fn init() {
    #[cfg(target_arch = "riscv64")]
    riscv64::init();
}

pub fn wait_for_interrupt() {
    #[cfg(target_arch = "riscv64")]
    riscv64::wait_for_interrupt();
}
