pub mod address;
pub mod frame;
pub mod memory_set;
pub mod page_table;

use lazy_static::lazy_static;
use spin::Mutex;
use alloc::sync::Arc;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum, PAGE_SIZE};
pub use memory_set::{MapArea, MapPerm, MapType, MemorySet};
pub use page_table::{PTEFlags, PageTable};

fn sym(f: unsafe extern "C" fn()) -> usize { f as *const () as usize }

lazy_static! {
    pub static ref KERNEL_SPACE: Arc<Mutex<MemorySet>> = {
        extern "C" {
            fn stext(); fn etext(); fn srodata(); fn erodata();
            fn sdata(); fn edata(); fn sbss(); fn ebss();
            fn ekernel();
        }
        let mut ms = MemorySet::new_bare();
        ms.push_identity(sym(stext), sym(etext), MapPerm::R | MapPerm::X);
        ms.push_identity(sym(srodata), sym(erodata), MapPerm::R);
        ms.push_identity(sym(sdata), sym(edata), MapPerm::R | MapPerm::W);
        ms.push_identity(sym(sbss), sym(ebss), MapPerm::R | MapPerm::W);
        ms.push_identity(sym(ekernel), 0x80000000 + 512 * 1024 * 1024, MapPerm::R | MapPerm::W);
        // Device MMIO
        ms.push_identity(0x0010_0000, 0x0010_1000, MapPerm::R | MapPerm::W); // test/poweroff
        ms.push_identity(0x1000_0000, 0x1000_1000, MapPerm::R | MapPerm::W); // uart
        ms.push_identity(0x1000_1000, 0x1000_9000, MapPerm::R | MapPerm::W); // virtio mmio
        ms.push_identity(0x0c00_0000, 0x1000_0000, MapPerm::R | MapPerm::W); // PLIC
        Arc::new(Mutex::new(ms))
    };
}

pub fn init() {
    frame::init_frame_allocator();
    let ks = KERNEL_SPACE.lock();
    crate::println!("[kernel] kernel space built, root ppn = {:#x}", ks.page_table.root_ppn.0);
    ks.activate();
    crate::println!("[kernel] satp written, paging on");
    drop(ks);
    crate::println!("[kernel] memory management initialized");
}
