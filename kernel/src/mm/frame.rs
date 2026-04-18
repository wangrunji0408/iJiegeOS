use super::address::{PhysAddr, PhysPageNum, PAGE_SIZE};
use alloc::vec::Vec;
use spin::Mutex;

pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    fn new(ppn: PhysPageNum) -> Self {
        // zero the frame
        let bytes = ppn.get_bytes_array();
        for b in bytes.iter_mut() { *b = 0; }
        Self { ppn }
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        FRAME_ALLOCATOR.lock().dealloc(self.ppn);
    }
}

struct StackFrameAllocator {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    const fn new() -> Self {
        Self { current: 0, end: 0, recycled: Vec::new() }
    }
    fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
    }
    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(p) = self.recycled.pop() {
            Some(PhysPageNum(p))
        } else if self.current == self.end {
            None
        } else {
            let p = self.current;
            self.current += 1;
            Some(PhysPageNum(p))
        }
    }
    fn dealloc(&mut self, ppn: PhysPageNum) {
        self.recycled.push(ppn.0);
    }
}

static FRAME_ALLOCATOR: Mutex<StackFrameAllocator> = Mutex::new(StackFrameAllocator::new());

pub fn init_frame_allocator() {
    extern "C" { fn ekernel(); }
    let start = PhysAddr(ekernel as *const () as usize).ceil();
    // qemu-virt default has RAM 0x80000000 - 0x80000000 + memory. We set 512 MiB
    let end = PhysAddr(0x80000000 + 512 * 1024 * 1024).floor();
    FRAME_ALLOCATOR.lock().init(start, end);
    let frames = end.0 - start.0;
    crate::println!(
        "[kernel] frame allocator: {:#x} .. {:#x} ({} frames, {} MiB)",
        start.0 * PAGE_SIZE,
        end.0 * PAGE_SIZE,
        frames,
        frames * PAGE_SIZE / 1024 / 1024
    );
}

pub fn alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc().map(FrameTracker::new)
}
