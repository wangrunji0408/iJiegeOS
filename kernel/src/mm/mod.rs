mod heap;
mod frame;
mod address;
mod page_table;
mod memory_set;

pub use address::{PhysAddr, VirtAddr, PhysPageNum, VirtPageNum, StepByOne};
pub use frame::{frame_alloc, frame_dealloc, FrameTracker};
pub use page_table::{PageTable, PTEFlags, PageTableEntry, translated_byte_buffer, translated_str, translated_refmut};
pub use memory_set::{MemorySet, MapPermission, KERNEL_SPACE};

/// 内核物理内存范围
/// QEMU virt machine: 128MB RAM starting at 0x80000000
pub const MEMORY_START: usize = 0x80000000;
pub const MEMORY_END: usize = 0x88000000; // 128MB

/// 内核加载地址
pub const KERNEL_BASE: usize = 0x80200000;

/// 用户空间虚拟地址范围
pub const USER_STACK_SIZE: usize = 1 << 20; // 1MB
pub const USER_STACK_TOP: usize = 0x8000_0000;   // 用户栈顶（虚拟）
pub const MMAP_BASE: usize = 0x0000_0001_0000_0000; // mmap区域基址（更高地址）

/// Trampoline页面（映射到最高虚拟地址）
pub const TRAMPOLINE: usize = usize::MAX - 4095;
/// 陷阱上下文页面
pub const TRAP_CONTEXT_BASE: usize = TRAMPOLINE - 4096;

pub fn init() {
    heap::init_heap();
    frame::init_frame_allocator();
    memory_set::init_kernel_space();
    log::info!("mm: Physical memory: {:#x} - {:#x}", MEMORY_START, MEMORY_END);
}

/// 处理页面错误
pub fn handle_page_fault(addr: usize, cause: usize) -> bool {
    // 尝试懒分配处理
    let task = crate::task::current_task();
    if let Some(task) = task {
        let mut inner = task.inner_exclusive_access();
        if let Some(area) = inner.memory_set.find_mmap_area(addr) {
            // 为mmap区域分配物理页
            inner.memory_set.handle_cow_fault(addr)
        } else {
            false
        }
    } else {
        false
    }
}
