use core::ptr::NonNull;
use virtio_drivers::{BufferDirection, Hal, PhysAddr as VirtioPhysAddr};

pub struct HalImpl;

unsafe impl Hal for HalImpl {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (VirtioPhysAddr, NonNull<u8>) {
        let frames = crate::mm::frame_alloc_contiguous(pages)
            .expect("DMA alloc failed");
        let pa = frames[0].ppn.addr().0;
        core::mem::forget(frames);
        crate::println!("[DMA] alloc {} pages at PA={:#x}", pages, pa);
        let ptr = unsafe { NonNull::new_unchecked(pa as *mut u8) };
        (pa, ptr)
    }

    unsafe fn dma_dealloc(_paddr: VirtioPhysAddr, _vaddr: NonNull<u8>, _pages: usize) -> i32 {
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: VirtioPhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new_unchecked(paddr as *mut u8)
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> VirtioPhysAddr {
        let addr = buffer.as_ptr() as *const u8 as VirtioPhysAddr;
        // Verify the address is in identity-mapped range
        if addr < 0x80000000 || addr >= 0x90000000 {
            crate::println!("[DMA] WARNING: share non-identity-mapped addr={:#x}", addr);
        }
        addr
    }

    unsafe fn unshare(_paddr: VirtioPhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
    }
}
