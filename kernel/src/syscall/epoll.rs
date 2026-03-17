/// epoll 和 select/poll 系统调用

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use lazy_static::lazy_static;
use crate::task::current_task;
use crate::mm::{translated_byte_buffer, translated_refmut};
use crate::timer::TimeSpec;
use super::errno::*;

/// epoll event 结构
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

// epoll 事件标志
const EPOLLIN:  u32 = 1;
const EPOLLOUT: u32 = 4;
const EPOLLERR: u32 = 8;
const EPOLLHUP: u32 = 16;
const EPOLLET:  u32 = 1 << 31;

// epoll_ctl 操作
const EPOLL_CTL_ADD: i32 = 1;
const EPOLL_CTL_DEL: i32 = 2;
const EPOLL_CTL_MOD: i32 = 3;

struct EpollEntry {
    fd: usize,
    events: u32,
    data: u64,
}

struct EpollInstance {
    entries: BTreeMap<usize, EpollEntry>,
}

/// 进程的 epoll 实例存储
struct EpollStore {
    instances: BTreeMap<usize, EpollInstance>,  // fd -> instance
}

lazy_static! {
    static ref EPOLL_STORE: Mutex<EpollStore> = Mutex::new(EpollStore {
        instances: BTreeMap::new(),
    });
}

pub struct EpollFd {
    id: usize,
}

impl crate::fs::FileDescriptor for EpollFd {
    fn read(&self, _: &mut [u8]) -> isize { -1 }
    fn write(&self, _: &[u8]) -> isize { -1 }
    fn stat(&self) -> crate::fs::FileStat {
        crate::fs::FileStat { st_mode: 0o100600, ..Default::default() }
    }
    fn is_readable(&self) -> bool { false }
    fn is_writable(&self) -> bool { false }
    fn epoll_id(&self) -> Option<usize> { Some(self.id) }
}

pub fn sys_epoll_create1(flags: i32) -> i64 {
    static EPOLL_ID: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(1000);
    let id = EPOLL_ID.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    EPOLL_STORE.lock().instances.insert(id, EpollInstance {
        entries: BTreeMap::new(),
    });

    let epoll_fd = Arc::new(EpollFd { id });
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let fd = inner.alloc_fd();
    inner.fd_table[fd] = Some(epoll_fd);
    fd as i64
}

pub fn sys_epoll_ctl(epfd: usize, op: i32, fd: usize, event: *const u8) -> i64 {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let epoll_file = match inner.get_fd(epfd) {
        Some(f) => f,
        None => return EBADF,
    };
    let epoll_id = match epoll_file.epoll_id() {
        Some(id) => id,
        None => return EINVAL,
    };
    drop(inner);

    // 读取事件
    let tok = crate::task::current_user_token();
    let evt = if !event.is_null() {
        let bufs = translated_byte_buffer(tok, event, core::mem::size_of::<EpollEvent>());
        let mut evt_bytes = [0u8; 12];
        let mut off = 0;
        for b in bufs {
            let end = (off + b.len()).min(12);
            evt_bytes[off..end].copy_from_slice(&b[..end-off]);
            off += b.len();
        }
        Some(unsafe { core::ptr::read_unaligned(evt_bytes.as_ptr() as *const EpollEvent) })
    } else {
        None
    };

    let mut store = EPOLL_STORE.lock();

    if let Some(instance) = store.instances.get_mut(&epoll_id) {
        match op {
            1 => {  // EPOLL_CTL_ADD
                if let Some(evt) = evt {
                    instance.entries.insert(fd, EpollEntry {
                        fd,
                        events: evt.events,
                        data: evt.data,
                    });
                }
            }
            2 => {  // EPOLL_CTL_DEL
                instance.entries.remove(&fd);
            }
            3 => {  // EPOLL_CTL_MOD
                if let Some(entry) = instance.entries.get_mut(&fd) {
                    if let Some(evt) = evt {
                        entry.events = evt.events;
                        entry.data = evt.data;
                    }
                }
            }
            _ => return EINVAL,
        }
    }
    0
}

pub fn sys_epoll_pwait(epfd: usize, events: *mut u8, maxevents: i32, timeout: i32, sigmask: *const u64) -> i64 {
    let tok = crate::task::current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();

    // 获取 epoll 实例 id
    let epoll_id = match inner.get_fd(epfd).and_then(|f| f.epoll_id()) {
        Some(id) => id,
        None => return EBADF,
    };

    // 轮询所有被监视的 fd
    let mut ready_events: Vec<EpollEvent> = Vec::new();

    // 先轮询网络
    drop(inner);
    crate::net::poll();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let store = EPOLL_STORE.lock();

    if let Some(instance) = store.instances.get(&epoll_id) {
        for (_, entry) in instance.entries.iter() {
            if let Some(file) = inner.get_fd(entry.fd) {
                let mut ready = 0u32;
                if entry.events & EPOLLIN != 0 && file.can_read() {
                    ready |= EPOLLIN;
                }
                if entry.events & EPOLLOUT != 0 && file.can_write() {
                    ready |= EPOLLOUT;
                }
                if ready != 0 {
                    ready_events.push(EpollEvent { events: ready, data: entry.data });
                }
            }
        }
    }
    drop(store);
    drop(inner);

    if ready_events.is_empty() && timeout == 0 {
        return 0;
    }

    if ready_events.is_empty() && timeout != 0 {
        // 如果有超时，等待一段时间
        let sleep_ms = if timeout < 0 { 100 } else { timeout.min(100) as u64 };
        let end = crate::timer::get_time_ms() + sleep_ms;
        while crate::timer::get_time_ms() < end {
            crate::task::suspend_current_and_run_next();
            crate::net::poll();

            // 重新检查
            let task = current_task().unwrap();
            let inner = task.inner_exclusive_access();
            let store = EPOLL_STORE.lock();
            if let Some(instance) = store.instances.get(&epoll_id) {
                for (_, entry) in instance.entries.iter() {
                    if let Some(file) = inner.get_fd(entry.fd) {
                        let mut ready = 0u32;
                        if entry.events & EPOLLIN != 0 && file.can_read() { ready |= EPOLLIN; }
                        if entry.events & EPOLLOUT != 0 && file.can_write() { ready |= EPOLLOUT; }
                        if ready != 0 {
                            ready_events.push(EpollEvent { events: ready, data: entry.data });
                        }
                    }
                }
            }
            if !ready_events.is_empty() { break; }
        }
    }

    let n = ready_events.len().min(maxevents as usize);
    let event_size = core::mem::size_of::<EpollEvent>();
    let bufs = translated_byte_buffer(tok, events, n * event_size);
    let mut off = 0;
    for b in bufs {
        let event_idx = off / event_size;
        if event_idx >= n { break; }
        let src = unsafe {
            core::slice::from_raw_parts(
                &ready_events[event_idx] as *const EpollEvent as *const u8,
                event_size
            )
        };
        b.copy_from_slice(&src[off % event_size..off % event_size + b.len()]);
        off += b.len();
    }
    n as i64
}

pub fn sys_pselect6(nfds: i32, readfds: usize, writefds: usize, exceptfds: usize, timeout: usize, sigmask: usize) -> i64 {
    let tok = crate::task::current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();

    let timeout_ms = if timeout != 0 {
        let ts = *crate::mm::translated_ref(tok, timeout as *const TimeSpec);
        ts.tv_sec as i64 * 1000 + ts.tv_nsec as i64 / 1_000_000
    } else {
        -1i64
    };

    // fd_set 是 128 字节的位图
    let get_fd_set = |ptr: usize| -> [u8; 128] {
        if ptr == 0 { return [0u8; 128]; }
        let mut set = [0u8; 128];
        let bufs = crate::mm::translated_byte_buffer(tok, ptr as *mut u8, 128);
        let mut off = 0;
        for b in bufs {
            set[off..off + b.len()].copy_from_slice(b);
            off += b.len();
        }
        set
    };

    let read_set = get_fd_set(readfds);
    let write_set = get_fd_set(writefds);

    crate::net::poll();

    let mut ready_read = [0u8; 128];
    let mut ready_write = [0u8; 128];
    let mut count = 0;

    for fd in 0..(nfds as usize) {
        let byte = fd / 8;
        let bit = fd % 8;
        if read_set[byte] & (1 << bit) != 0 {
            if let Some(file) = inner.get_fd(fd) {
                if file.can_read() {
                    ready_read[byte] |= 1 << bit;
                    count += 1;
                }
            }
        }
        if write_set[byte] & (1 << bit) != 0 {
            if let Some(file) = inner.get_fd(fd) {
                if file.can_write() {
                    ready_write[byte] |= 1 << bit;
                    count += 1;
                }
            }
        }
    }
    drop(inner);

    // 写回结果
    if readfds != 0 {
        let bufs = crate::mm::translated_byte_buffer(tok, readfds as *mut u8, 128);
        let mut off = 0;
        for b in bufs {
            b.copy_from_slice(&ready_read[off..off + b.len()]);
            off += b.len();
        }
    }
    if writefds != 0 {
        let bufs = crate::mm::translated_byte_buffer(tok, writefds as *mut u8, 128);
        let mut off = 0;
        for b in bufs {
            b.copy_from_slice(&ready_write[off..off + b.len()]);
            off += b.len();
        }
    }

    count
}

pub fn sys_ppoll(fds: *mut u8, nfds: usize, timeout: *const TimeSpec, sigmask: *const u64) -> i64 {
    let tok = crate::task::current_user_token();

    // struct pollfd { int fd, short events, short revents }
    let pollfd_size = 8;
    let mut kernel_fds = alloc::vec![0u8; nfds * pollfd_size];
    let bufs = translated_byte_buffer(tok, fds, nfds * pollfd_size);
    let mut off = 0;
    for b in bufs {
        kernel_fds[off..off + b.len()].copy_from_slice(b);
        off += b.len();
    }

    crate::net::poll();

    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let mut count = 0i64;

    for i in 0..nfds {
        let base = i * pollfd_size;
        let fd = i32::from_le_bytes([
            kernel_fds[base], kernel_fds[base+1],
            kernel_fds[base+2], kernel_fds[base+3]
        ]);
        let events = i16::from_le_bytes([kernel_fds[base+4], kernel_fds[base+5]]);

        let mut revents = 0i16;
        if fd >= 0 {
            if let Some(file) = inner.get_fd(fd as usize) {
                if events & 1 != 0 && file.can_read() { revents |= 1; }  // POLLIN
                if events & 4 != 0 && file.can_write() { revents |= 4; } // POLLOUT
                if file.has_error() { revents |= 8; }  // POLLERR
            }
        }
        kernel_fds[base+6] = (revents & 0xff) as u8;
        kernel_fds[base+7] = ((revents >> 8) & 0xff) as u8;
        if revents != 0 { count += 1; }
    }
    drop(inner);

    // 写回结果
    let bufs = translated_byte_buffer(tok, fds, nfds * pollfd_size);
    let mut off = 0;
    for b in bufs {
        b.copy_from_slice(&kernel_fds[off..off + b.len()]);
        off += b.len();
    }

    count
}
