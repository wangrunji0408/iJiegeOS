use buddy_system_allocator::LockedHeap;

const KERNEL_HEAP_SIZE: usize = 0x80_0000; // 8 MiB

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

pub fn init() {
    unsafe {
        HEAP.lock().init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

#[alloc_error_handler]
fn oom(layout: core::alloc::Layout) -> ! {
    panic!("heap allocation error: {:?}", layout);
}
