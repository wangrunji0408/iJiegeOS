//! Virtio HAL used by virtio-drivers. Physical memory is identity mapped.
use crate::mm::address::PAGE_SIZE;
use crate::mm::frame::{alloc, FrameTracker};
use alloc::vec::Vec;
use core::ptr::NonNull;
use spin::Mutex;
use virtio_drivers::{BufferDirection, Hal, PhysAddr};

/// A bag of frames pinned in memory for virtio DMA buffers.
static DMA_FRAMES: Mutex<Vec<FrameTracker>> = Mutex::new(Vec::new());

pub struct KernelHal;

unsafe impl Hal for KernelHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let mut frames = Vec::with_capacity(pages);
        let first_ppn;
        {
            let f0 = alloc().expect("virtio: no frames");
            first_ppn = f0.ppn.0;
            frames.push(f0);
            for i in 1..pages {
                let f = alloc().expect("virtio: no frames");
                // best-effort contiguity — bail if fragmented
                assert_eq!(f.ppn.0, first_ppn + i, "virtio dma_alloc needs contiguous frames");
                frames.push(f);
            }
        }
        DMA_FRAMES.lock().extend(frames);
        let pa = first_ppn * PAGE_SIZE;
        let ptr = NonNull::new(pa as *mut u8).unwrap();
        (pa, ptr)
    }

    unsafe fn dma_dealloc(_pa: PhysAddr, _vaddr: NonNull<u8>, _pages: usize) -> i32 {
        // Frames stay alive for the lifetime of the driver (demo: no recycling).
        0
    }

    unsafe fn mmio_phys_to_virt(pa: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(pa as *mut u8).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        buffer.as_ptr() as *const u8 as usize
    }

    unsafe fn unshare(_pa: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}
