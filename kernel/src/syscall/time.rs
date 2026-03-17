/// 时间相关系统调用

use crate::timer::{TimeSpec, TimeVal, get_time_us};
use crate::mm::translated_refmut;
use super::errno::*;

fn token() -> usize {
    crate::task::current_user_token()
}

pub fn sys_clock_gettime(clockid: i32, tp: *mut TimeSpec) -> i64 {
    let ts = match clockid {
        0 | 1 | 4 => TimeSpec::now(),  // CLOCK_REALTIME, CLOCK_MONOTONIC, CLOCK_MONOTONIC_RAW
        _ => TimeSpec::now(),
    };
    if !tp.is_null() {
        *translated_refmut(token(), tp) = ts;
    }
    0
}

pub fn sys_clock_getres(clockid: i32, tp: *mut TimeSpec) -> i64 {
    if !tp.is_null() {
        *translated_refmut(token(), tp) = TimeSpec { tv_sec: 0, tv_nsec: 1 };
    }
    0
}

pub fn sys_clock_nanosleep(clockid: i32, flags: i32, request: *const TimeSpec, remain: *mut TimeSpec) -> i64 {
    if request.is_null() { return EINVAL; }
    let ts = *crate::mm::translated_ref(token(), request);
    sys_nanosleep(&ts as *const TimeSpec, remain)
}

pub fn sys_nanosleep(req: *const TimeSpec, rem: *mut TimeSpec) -> i64 {
    if req.is_null() { return EINVAL; }
    let ts = *crate::mm::translated_ref(token(), req);
    let sleep_us = ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1000;
    let start = get_time_us();
    let end = start + sleep_us;

    // 通过多次让出 CPU 来模拟睡眠
    while get_time_us() < end {
        crate::task::suspend_current_and_run_next();
    }

    if !rem.is_null() {
        *translated_refmut(token(), rem) = TimeSpec { tv_sec: 0, tv_nsec: 0 };
    }
    0
}

pub fn sys_gettimeofday(tv: *mut TimeVal, tz: usize) -> i64 {
    if !tv.is_null() {
        *translated_refmut(token(), tv) = TimeVal::now();
    }
    0
}

pub fn sys_times(buf: usize) -> i64 {
    // struct tms { utime, stime, cutime, cstime } 各 clock_t
    if buf != 0 {
        let bufs = crate::mm::translated_byte_buffer(token(), buf as *mut u8, 32);
        for b in bufs {
            for byte in b.iter_mut() { *byte = 0; }
        }
    }
    crate::timer::get_ticks() as i64
}
