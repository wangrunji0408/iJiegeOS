pub mod address;
pub mod frame;
pub mod memory_set;
pub mod page_table;

use lazy_static::lazy_static;
use spin::Mutex;
use alloc::sync::Arc;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum, PAGE_SIZE};
pub use memory_set::{MapPerm, MemorySet};
pub use page_table::{PTEFlags, PageTable};

lazy_static! {
    pub static ref KERNEL_SPACE: Arc<Mutex<MemorySet>> = {
        let mut ms = MemorySet::new_bare();
        extern "C" {
            fn stext(); fn etext(); fn srodata(); fn erodata();
            fn sdata(); fn edata(); fn sbss(); fn ebss();
            fn ekernel();
        }
        ms.push_identity(VirtAddr(stext as usize), VirtAddr(etext as usize), MapPerm::R | MapPerm::X);
        ms.push_identity(VirtAddr(srodata as usize), VirtAddr(erodata as usize), MapPerm::R);
        ms.push_identity(VirtAddr(sdata as usize), VirtAddr(edata as usize), MapPerm::R | MapPerm::W);
        ms.push_identity(VirtAddr(sbss as usize), VirtAddr(ebss as usize), MapPerm::R | MapPerm::W);
        ms.push_identity(VirtAddr(ekernel as usize), VirtAddr(0x80000000 + 512 * 1024 * 1024), MapPerm::R | MapPerm::W);
        ms.push_identity(VirtAddr(0x10001000), VirtAddr(0x10009000), MapPerm::R | MapPerm::W);
        ms.push_identity(VirtAddr(0x0c000000), VirtAddr(0x10000000), MapPerm::R | MapPerm::W);
        ms.push_identity(VirtAddr(0x10000000), VirtAddr(0x10001000), MapPerm::R | MapPerm::W);
        Arc::new(Mutex::new(ms))
    };
}

pub fn init() {
    frame::init_frame_allocator();
    KERNEL_SPACE.lock().activate();
    crate::println!("[kernel] memory management initialized");
}
