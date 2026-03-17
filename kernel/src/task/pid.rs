use spin::Mutex;
use core::sync::atomic::{AtomicI32, Ordering};

/// PID 类型
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pid(pub i32);

/// PID 分配器
pub struct PidAllocator {
    next: AtomicI32,
}

impl PidAllocator {
    pub const fn new() -> Self {
        Self { next: AtomicI32::new(1) }
    }

    pub fn alloc(&self) -> Pid {
        Pid(self.next.fetch_add(1, Ordering::SeqCst))
    }
}

pub static PID_ALLOCATOR: PidAllocator = PidAllocator::new();
