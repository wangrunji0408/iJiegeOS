mod heap;
mod frame;
mod page_table;
mod address;
mod memory_set;

pub use address::*;
pub use frame::*;
pub use page_table::*;
pub use memory_set::*;

pub fn init() {
    heap::init();
    frame::init();
    // Enable paging with kernel page table
    let satp = KERNEL_SPACE.lock().token();
    unsafe {
        riscv::register::satp::write(satp);
        core::arch::asm!("sfence.vma");
    }
    println!("[MM] Kernel page table activated, satp={:#x}", satp);
}
