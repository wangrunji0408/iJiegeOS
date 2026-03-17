use riscv::register::time;
use crate::arch::sbi::set_timer;

/// 定时器频率（QEMU virt machine = 10MHz）
pub const CLOCK_FREQ: u64 = 10_000_000;
/// 每次定时器中断的间隔（时间片 = 10ms）
pub const TICKS_PER_SEC: u64 = 100;
pub const TICK_INTERVAL: u64 = CLOCK_FREQ / TICKS_PER_SEC;

/// Unix 时间戳基准偏移（2024-01-01 00:00:00 UTC）
/// 使 gettimeofday 返回合理的 Unix 时间戳，避免 nginx 等程序
/// 因时间为 0 而跳过时间初始化（ngx_cached_err_log_time.data 不被设置）
pub const UNIX_EPOCH_OFFSET: u64 = 1704067200;

/// 全局 tick 计数
static TICK_COUNT: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

pub fn init() {
    set_next_timer();
    log::info!("timer: initialized, freq={}Hz, tick={}ms",
        CLOCK_FREQ, 1000 / TICKS_PER_SEC);
}

pub fn get_time() -> u64 {
    time::read64()
}

pub fn get_time_ms() -> u64 {
    time::read64() * 1000 / CLOCK_FREQ
}

pub fn get_time_us() -> u64 {
    time::read64() * 1_000_000 / CLOCK_FREQ
}

pub fn get_ticks() -> u64 {
    TICK_COUNT.load(core::sync::atomic::Ordering::Relaxed)
}

fn set_next_timer() {
    let next = time::read64() + TICK_INTERVAL;
    set_timer(next);
}

pub fn handle_timer_interrupt() {
    set_next_timer();
    TICK_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
}

/// 时间结构体（与 Linux 兼容）
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeVal {
    pub tv_sec: u64,
    pub tv_usec: u64,
}

impl TimeVal {
    pub fn now() -> Self {
        let us = get_time_us();
        Self {
            tv_sec: UNIX_EPOCH_OFFSET + us / 1_000_000,
            tv_usec: us % 1_000_000,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeSpec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

impl TimeSpec {
    pub fn now() -> Self {
        let us = get_time_us();
        Self {
            tv_sec: (UNIX_EPOCH_OFFSET + us / 1_000_000) as i64,
            tv_nsec: (us % 1_000_000 * 1000) as i64,
        }
    }

    pub fn from_us(us: u64) -> Self {
        Self {
            tv_sec: (us / 1_000_000) as i64,
            tv_nsec: (us % 1_000_000 * 1000) as i64,
        }
    }

    pub fn to_us(&self) -> u64 {
        self.tv_sec as u64 * 1_000_000 + self.tv_nsec as u64 / 1000
    }
}
