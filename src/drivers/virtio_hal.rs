use virtio_drivers::{BufferDirection, Hal, PhysAddr as VirtioPhysAddr, PAGE_SIZE};
use crate::mm::{PhysAddr, frame_alloc, PhysPageNum};

pub struct HalImpl;

unsafe impl Hal for HalImpl {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (VirtioPhysAddr, core::ptr::NonNull<u8>) {
        let frames = crate::mm::frame_alloc_contiguous(pages)
            .expect("DMA alloc failed");
        let pa = frames[0].ppn.addr().0;
        core::mem::forget(frames);
        let ptr = unsafe { core::ptr::NonNull::new_unchecked(pa as *mut u8) };
        (pa, ptr)
    }

    unsafe fn dma_dealloc(paddr: VirtioPhysAddr, _vaddr: core::ptr::NonNull<u8>, _pages: usize) -> i32 {
        // TODO: properly deallocate
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: VirtioPhysAddr, _size: usize) -> core::ptr::NonNull<u8> {
        // Identity mapping
        core::ptr::NonNull::new_unchecked(paddr as *mut u8)
    }

    unsafe fn share(buffer: core::ptr::NonNull<[u8]>, _direction: BufferDirection) -> VirtioPhysAddr {
        // Identity mapping: VA == PA for kernel memory
        buffer.as_ptr() as *const u8 as VirtioPhysAddr
    }

    unsafe fn unshare(_paddr: VirtioPhysAddr, _buffer: core::ptr::NonNull<[u8]>, _direction: BufferDirection) {
        // No-op for identity mapping
    }
}
