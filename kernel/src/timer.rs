use core::sync::atomic::{AtomicUsize, Ordering};

const MS_PER_SEC: usize = 1000;
const CLOCK_FREQ: usize = 10_000_000; // qemu virt clint frequency

static TICKS: AtomicUsize = AtomicUsize::new(0);

pub fn tick() {
    TICKS.fetch_add(1, Ordering::SeqCst);
    set_next_trigger();
}

pub fn init() {
    set_next_trigger();
}

/// nanoseconds since boot
pub fn now_ns() -> u64 {
    let t = read_time();
    (t as u64).saturating_mul(1_000_000_000 / CLOCK_FREQ as u64)
}

pub fn now_us() -> u64 { now_ns() / 1000 }
pub fn now_ms() -> u64 { now_ns() / 1_000_000 }
pub fn now_sec() -> u64 { now_ns() / 1_000_000_000 }

pub fn read_time() -> usize {
    let t: usize;
    unsafe { core::arch::asm!("rdtime {}", out(reg) t); }
    t
}

fn set_next_trigger() {
    let next = read_time() + CLOCK_FREQ / MS_PER_SEC * 10; // 10 ms
    sbi_set_timer(next as u64);
}

fn sbi_set_timer(t: u64) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 0x54494D45usize, // TIME extension
            in("a6") 0usize,
            in("a0") t as usize,
        );
    }
}
