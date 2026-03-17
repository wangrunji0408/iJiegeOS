mod heap;
mod frame;
mod address;
mod page_table;
mod memory_set;

pub use address::{PhysAddr, VirtAddr, PhysPageNum, VirtPageNum, StepByOne, VPNRange, PAGE_SIZE};
pub use frame::{frame_alloc, frame_dealloc, FrameTracker};
pub use page_table::{PageTable, PTEFlags, PageTableEntry, translated_byte_buffer, translated_str, translated_refmut, translated_ref};
pub use memory_set::{MemorySet, MapArea, MapType, MapPermission, MmapArea, KERNEL_SPACE};
pub use memory_set::activate_kernel_space;

/// 物理内存范围（QEMU virt machine 128MB from 0x80000000）
pub const MEMORY_START: usize = 0x80000000;
pub const MEMORY_END: usize = 0x88000000;

pub fn init() {
    crate::println!("mm: init heap...");
    heap::init_heap();
    crate::println!("mm: init frame allocator...");
    frame::init_frame_allocator();
    crate::println!("mm: init kernel space...");
    memory_set::init_kernel_space();
    crate::println!("mm: done");
}

/// 处理页面错误（懒分配）
pub fn handle_page_fault(addr: usize, cause: usize) -> bool {
    if let Some(task) = crate::task::current_task() {
        let mut inner = task.inner_exclusive_access();
        inner.memory_set.handle_cow_fault(addr)
    } else {
        false
    }
}
