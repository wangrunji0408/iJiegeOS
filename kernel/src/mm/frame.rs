use super::address::{PhysAddr, PhysPageNum};
use alloc::vec::Vec;
use spin::Mutex;

/// 物理页帧
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        // 清零页面
        ppn.get_bytes_array().fill(0);
        Self { ppn }
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

/// 栈式帧分配器
pub struct StackFrameAllocator {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
        log::info!("frame allocator: ppn [{:#x}, {:#x}), {} frames",
            l.0, r.0, r.0 - l.0);
    }
}

impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else if self.current == self.end {
            None
        } else {
            self.current += 1;
            Some((self.current - 1).into())
        }
    }

    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // 检查是否是有效的已分配页
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("frame dealloc: ppn {:#x} is invalid", ppn);
        }
        self.recycled.push(ppn);
    }
}

static FRAME_ALLOCATOR: Mutex<StackFrameAllocator> = Mutex::new(StackFrameAllocator {
    current: 0,
    end: 0,
    recycled: Vec::new(),
});

pub fn init_frame_allocator() {
    extern "C" {
        fn ekernel();
    }
    let kernel_end = PhysAddr::from(ekernel as usize).ceil();
    let memory_end = PhysAddr::from(super::MEMORY_END).floor();
    FRAME_ALLOCATOR.lock().init(kernel_end, memory_end);
}

pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc().map(FrameTracker::new)
}

pub fn frame_alloc_contiguous(n: usize) -> Option<Vec<FrameTracker>> {
    let mut frames = Vec::new();
    for _ in 0..n {
        match frame_alloc() {
            Some(f) => frames.push(f),
            None => return None,
        }
    }
    Some(frames)
}

pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.lock().dealloc(ppn);
}
