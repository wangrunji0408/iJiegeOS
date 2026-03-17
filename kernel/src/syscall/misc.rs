/// 杂项系统调用

use crate::mm::translated_byte_buffer;
use super::errno::*;

fn token() -> usize {
    crate::task::current_user_token()
}

/// uname 结构
#[repr(C)]
#[derive(Clone, Copy)]
pub struct UtsName {
    pub sysname:    [u8; 65],
    pub nodename:   [u8; 65],
    pub release:    [u8; 65],
    pub version:    [u8; 65],
    pub machine:    [u8; 65],
    pub domainname: [u8; 65],
}

pub fn sys_uname(buf: *mut u8) -> i64 {
    let mut uname = UtsName {
        sysname: [0; 65],
        nodename: [0; 65],
        release: [0; 65],
        version: [0; 65],
        machine: [0; 65],
        domainname: [0; 65],
    };

    copy_str(&mut uname.sysname, b"Linux");
    copy_str(&mut uname.nodename, b"ijiege");
    copy_str(&mut uname.release, b"5.15.0-iJiege");
    copy_str(&mut uname.version, b"#1 SMP Mon Mar 17 00:00:00 2026");
    copy_str(&mut uname.machine, b"riscv64");
    copy_str(&mut uname.domainname, b"(none)");

    let bytes = unsafe {
        core::slice::from_raw_parts(
            &uname as *const UtsName as *const u8,
            core::mem::size_of::<UtsName>()
        )
    };

    let bufs = translated_byte_buffer(token(), buf, bytes.len());
    let mut off = 0;
    for b in bufs {
        b.copy_from_slice(&bytes[off..off + b.len()]);
        off += b.len();
    }
    0
}

fn copy_str(dst: &mut [u8], src: &[u8]) {
    let n = src.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&src[..n]);
    dst[n] = 0;
}

/// sysinfo 结构
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SysInfo {
    pub uptime: i64,
    pub loads: [u64; 3],
    pub totalram: u64,
    pub freeram: u64,
    pub sharedram: u64,
    pub bufferram: u64,
    pub totalswap: u64,
    pub freeswap: u64,
    pub procs: u16,
    pub pad: u16,
    pub pad2: u32,
    pub totalhigh: u64,
    pub freehigh: u64,
    pub mem_unit: u32,
    pub _f: [u8; 8],
}

pub fn sys_sysinfo(buf: *mut u8) -> i64 {
    let info = SysInfo {
        uptime: (crate::timer::get_time_ms() / 1000) as i64,
        totalram: 128 * 1024 * 1024,
        freeram: 64 * 1024 * 1024,
        procs: 1,
        mem_unit: 1,
        ..Default::default()
    };

    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const SysInfo as *const u8,
            core::mem::size_of::<SysInfo>()
        )
    };

    let bufs = translated_byte_buffer(token(), buf, bytes.len());
    let mut off = 0;
    for b in bufs {
        b.copy_from_slice(&bytes[off..off + b.len()]);
        off += b.len();
    }
    0
}

pub fn sys_getrandom(buf: *mut u8, len: usize, flags: u32) -> i64 {
    static SEED: spin::Mutex<u64> = spin::Mutex::new(0xdeadbeef);
    let mut seed = SEED.lock();
    let bufs = translated_byte_buffer(token(), buf, len);
    let mut written = 0;
    for b in bufs {
        for byte in b.iter_mut() {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            *byte = (*seed >> 33) as u8;
            written += 1;
        }
    }
    written as i64
}

pub fn sys_sched_getaffinity(pid: usize, cpusetsize: usize, mask: *mut u8) -> i64 {
    if cpusetsize < 8 { return EINVAL; }
    // 单核系统，只有 CPU 0
    let bufs = translated_byte_buffer(token(), mask, 8.min(cpusetsize));
    let mut first = true;
    for b in bufs {
        for byte in b.iter_mut() {
            *byte = if first { 1 } else { 0 };
            first = false;
        }
    }
    0
}

pub fn sys_futex(uaddr: usize, op: i32, val: u32, timeout: usize, uaddr2: usize, val3: u32) -> i64 {
    let futex_op = op & 0x7f;  // 去掉 FUTEX_PRIVATE_FLAG
    match futex_op {
        0 => {  // FUTEX_WAIT
            let tok = token();
            let current = *crate::mm::translated_ref(tok, uaddr as *const u32);
            if current != val {
                return EAGAIN;
            }
            // 让出 CPU，返回 EINTR 让 musl 认为被信号中断而不是无限重试
            crate::task::suspend_current_and_run_next();
            EINTR
        }
        1 => {  // FUTEX_WAKE
            0  // 没有 wake 任何东西
        }
        _ => 0,
    }
}

pub fn sys_eventfd2(initval: u32, flags: i32) -> i64 {
    let efd = alloc::sync::Arc::new(EventFd::new(initval, flags));
    let task = crate::task::current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let fd = inner.alloc_fd();
    inner.fd_table[fd] = Some(efd);
    fd as i64
}

/// eventfd 实现：内核计数器，支持 read/write/can_read
struct EventFd {
    counter: spin::Mutex<u64>,
    semaphore: bool,  // EFD_SEMAPHORE flag
}

impl EventFd {
    fn new(initval: u32, flags: i32) -> Self {
        Self {
            counter: spin::Mutex::new(initval as u64),
            semaphore: flags & 1 != 0,  // EFD_SEMAPHORE = 1
        }
    }
}

impl crate::fs::FileDescriptor for EventFd {
    fn read(&self, buf: &mut [u8]) -> isize {
        if buf.len() < 8 { return -22; }  // EINVAL
        let mut counter = self.counter.lock();
        if *counter == 0 {
            return -11;  // EAGAIN
        }
        let val = if self.semaphore { 1 } else { *counter };
        *counter -= val;
        let bytes = val.to_ne_bytes();
        buf[..8].copy_from_slice(&bytes);
        8
    }

    fn write(&self, buf: &[u8]) -> isize {
        if buf.len() < 8 { return -22; }  // EINVAL
        let mut val_bytes = [0u8; 8];
        val_bytes.copy_from_slice(&buf[..8]);
        let val = u64::from_ne_bytes(val_bytes);
        if val == u64::MAX { return -22; }  // EINVAL
        let mut counter = self.counter.lock();
        *counter = counter.saturating_add(val);
        8
    }

    fn stat(&self) -> crate::fs::FileStat {
        crate::fs::FileStat { st_mode: 0o20600, ..Default::default() }
    }

    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { true }

    fn can_read(&self) -> bool {
        *self.counter.lock() > 0
    }

    fn can_write(&self) -> bool { true }

    fn ioctl(&self, request: u64, arg: usize) -> isize {
        // FIONBIO (0x5421) - 设置/清除非阻塞模式，对 eventfd 忽略即可
        0
    }

    fn set_nonblock(&self, _nonblock: bool) {}
    fn is_nonblock(&self) -> bool { true }  // eventfd 总是非阻塞
}

pub fn sys_timerfd_create(clockid: i32, flags: i32) -> i64 {
    // 返回一个简单的 pipe 读端作为 timerfd
    let (r, w) = crate::fs::create_pipe();
    let task = crate::task::current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let fd = inner.alloc_fd();
    inner.fd_table[fd] = Some(r);
    fd as i64
}

pub fn sys_inotify_init1(flags: i32) -> i64 {
    let (r, w) = crate::fs::create_pipe();
    let task = crate::task::current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let fd = inner.alloc_fd();
    inner.fd_table[fd] = Some(r);
    fd as i64
}

fn readlink_at(path: &str) -> Option<alloc::string::String> {
    crate::fs::readlink(path)
}
