use alloc::vec::Vec;
use spin::Mutex;
use crate::config::PAGE_SIZE;
use super::{PhysAddr, PhysPageNum};

/// A tracker for a physical frame that deallocates on drop
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        // Clear the frame using volatile writes to avoid optimization issues
        let addr = ppn.addr().0 as *mut u64;
        for i in 0..(PAGE_SIZE / 8) {
            unsafe { core::ptr::write_volatile(addr.add(i), 0); }
        }
        Self { ppn }
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        FRAME_ALLOCATOR.lock().dealloc(self.ppn);
    }
}

struct StackFrameAllocator {
    current: usize,  // next available physical page number
    end: usize,      // end of available physical pages
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    const fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }

    fn init(&mut self, start: PhysPageNum, end: PhysPageNum) {
        self.current = start.0;
        self.end = end.0;
        println!("[MM] Frame allocator: {:#x} - {:#x}, {} frames available",
            start.addr().0, end.addr().0, end.0 - start.0);
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(PhysPageNum(ppn))
        } else if self.current < self.end {
            let ppn = self.current;
            self.current += 1;
            Some(PhysPageNum(ppn))
        } else {
            None
        }
    }

    fn dealloc(&mut self, ppn: PhysPageNum) {
        // Validity check
        if ppn.0 >= self.current || self.recycled.contains(&ppn.0) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn.0);
        }
        self.recycled.push(ppn.0);
    }

    fn available(&self) -> usize {
        self.end - self.current + self.recycled.len()
    }
}

static FRAME_ALLOCATOR: Mutex<StackFrameAllocator> = Mutex::new(StackFrameAllocator::new());

pub fn init() {
    extern "C" {
        fn ekernel();
    }
    let start = PhysAddr(ekernel as usize).ceil();
    let end = PhysAddr(crate::config::MEMORY_END).floor();
    FRAME_ALLOCATOR.lock().init(start, end);
}

pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc().map(FrameTracker::new)
}

pub fn frame_alloc_contiguous(count: usize) -> Option<Vec<FrameTracker>> {
    let mut allocator = FRAME_ALLOCATOR.lock();
    // Simple: allocate contiguous from current
    if allocator.current + count <= allocator.end {
        let start = allocator.current;
        allocator.current += count;
        drop(allocator);
        Some((start..start + count).map(|ppn| FrameTracker::new(PhysPageNum(ppn))).collect())
    } else {
        None
    }
}

pub fn frame_available() -> usize {
    FRAME_ALLOCATOR.lock().available()
}
