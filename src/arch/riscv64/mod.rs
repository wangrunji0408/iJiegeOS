pub mod sbi;

pub use sbi::*;

/// Shutdown the machine
pub fn shutdown() -> ! {
    sbi::shutdown();
}
