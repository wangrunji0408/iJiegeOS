use crate::syscall::*;
use crate::trap::TrapContext;
use crate::mm::page_table::{read_user_bytes, write_user_bytes};
use crate::mm::address::{VirtAddr, VirtPageNum, PAGE_SIZE};
use crate::mm::memory_set::{MapArea, MapPerm, MapType};
use crate::mm::frame::alloc as alloc_frame;
use alloc::sync::Arc;
use alloc::string::String;

pub fn dispatch(id: usize, args: [usize; 6], _cx: &mut TrapContext) -> isize {
    // Trace every syscall (disabled by default to avoid flooding)
    static TRACE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    if TRACE.load(core::sync::atomic::Ordering::Relaxed) {
        let name = match id {
            SYS_WRITE => "write", SYS_READ => "read", SYS_WRITEV => "writev", SYS_READV => "readv",
            SYS_CLOSE => "close", SYS_OPENAT => "openat", SYS_FSTAT => "fstat",
            SYS_NEWFSTATAT => "newfstatat", SYS_LSEEK => "lseek", SYS_PREAD64 => "pread",
            SYS_MMAP => "mmap", SYS_MUNMAP => "munmap", SYS_MPROTECT => "mprotect",
            SYS_BRK => "brk", SYS_RT_SIGACTION => "rt_sigaction",
            SYS_RT_SIGPROCMASK => "rt_sigprocmask", SYS_UNAME => "uname",
            SYS_GETRANDOM => "getrandom", SYS_PRLIMIT64 => "prlimit64",
            SYS_IOCTL => "ioctl", SYS_FCNTL => "fcntl", SYS_SOCKET => "socket",
            SYS_BIND => "bind", SYS_LISTEN => "listen", SYS_ACCEPT => "accept",
            SYS_EXIT => "exit", SYS_EXIT_GROUP => "exit_group",
            SYS_SET_TID_ADDRESS => "set_tid", SYS_CLOCK_GETTIME => "clock_gettime",
            SYS_GETTIMEOFDAY => "gettimeofday", SYS_GETPID => "getpid",
            SYS_READLINKAT => "readlinkat", SYS_FACCESSAT => "faccessat",
            SYS_GETCWD => "getcwd", SYS_CHDIR => "chdir",
            _ => "?",
        };
        crate::println!("[sc] {}({}) {:#x?}", name, id, args);
    }
    dispatch_inner(id, args, _cx)
}

fn dispatch_inner(id: usize, args: [usize; 6], _cx: &mut TrapContext) -> isize {
    match id {
        SYS_WRITE => sys_write(args[0] as i32, args[1], args[2]),
        SYS_READ => sys_read(args[0] as i32, args[1], args[2]),
        SYS_WRITEV => sys_writev(args[0] as i32, args[1], args[2]),
        SYS_READV => sys_readv(args[0] as i32, args[1], args[2]),
        SYS_CLOSE => sys_close(args[0] as i32),
        SYS_EXIT | SYS_EXIT_GROUP => crate::task::exit_current(args[0] as i32),
        SYS_GETPID => crate::task::current().pid as isize,
        SYS_GETPPID => 0,
        SYS_GETUID | SYS_GETEUID | SYS_GETGID | SYS_GETEGID => 0,
        SYS_GETTID => crate::task::current().pid as isize,
        SYS_SCHED_YIELD => { crate::task::yield_current(); 0 }
        SYS_BRK => sys_brk(args[0]),
        SYS_SET_TID_ADDRESS | SYS_SET_ROBUST_LIST => 0,
        SYS_RT_SIGACTION | SYS_RT_SIGPROCMASK | SYS_RT_SIGRETURN => 0,
        SYS_UNAME => sys_uname(args[0]),
        SYS_CLOCK_GETTIME => sys_clock_gettime(args[0], args[1]),
        SYS_GETTIMEOFDAY => sys_gettimeofday(args[0]),
        SYS_OPENAT => sys_openat(args[0] as i32, args[1], args[2] as u32, args[3] as u32),
        SYS_FSTAT => sys_fstat(args[0] as i32, args[1]),
        SYS_NEWFSTATAT => sys_newfstatat(args[0] as i32, args[1], args[2], args[3] as i32),
        SYS_LSEEK => sys_lseek(args[0] as i32, args[1] as isize, args[2] as u32),
        SYS_PREAD64 => sys_pread(args[0] as i32, args[1], args[2], args[3] as u64),
        SYS_READLINKAT => sys_readlinkat(args[0] as i32, args[1], args[2], args[3]),
        SYS_GETCWD => sys_getcwd(args[0], args[1]),
        SYS_CHDIR => 0,
        SYS_FACCESSAT => sys_faccessat(args[0] as i32, args[1], args[2] as i32, args[3] as i32),
        SYS_MMAP => sys_mmap(args[0], args[1], args[2] as i32, args[3] as i32, args[4] as i32, args[5] as u64),
        SYS_MUNMAP => sys_munmap(args[0], args[1]),
        SYS_MPROTECT => sys_mprotect(args[0], args[1], args[2] as i32),
        SYS_MADVISE => 0,
        SYS_GETRANDOM => sys_getrandom(args[0], args[1], args[2] as u32),
        SYS_PRLIMIT64 => sys_prlimit64(args[0], args[1] as i32, args[2], args[3]),
        SYS_IOCTL => sys_ioctl(args[0] as i32, args[1] as u32, args[2]),
        SYS_FCNTL => sys_fcntl(args[0] as i32, args[1] as u32, args[2]),
        SYS_DUP => sys_dup(args[0] as i32),
        SYS_DUP3 => sys_dup3(args[0] as i32, args[1] as i32, args[2] as u32),
        SYS_SOCKET => sys_socket(args[0] as i32, args[1] as i32, args[2] as i32),
        SYS_BIND => sys_bind(args[0] as i32, args[1], args[2] as u32),
        SYS_LISTEN => sys_listen(args[0] as i32, args[1] as i32),
        SYS_ACCEPT | SYS_ACCEPT4 => sys_accept(args[0] as i32, args[1], args[2], args[3] as i32),
        SYS_SETSOCKOPT | SYS_GETSOCKOPT => 0,
        SYS_SHUTDOWN => sys_shutdown(args[0] as i32),
        SYS_PIPE2 => sys_pipe2(args[0], args[1] as u32),
        SYS_SIGNALFD4 => 0,
        SYS_EVENTFD2 => sys_eventfd2(args[0] as u32, args[1] as u32),
        SYS_EPOLL_CREATE1 => sys_epoll_create1(args[0] as u32),
        SYS_EPOLL_CTL => 0,
        SYS_EPOLL_PWAIT => 0,
        SYS_TIMERFD_CREATE => -38,
        SYS_TGKILL | SYS_TKILL | SYS_KILL => 0,
        SYS_FUTEX => 0,
        SYS_UMASK => 0,
        SYS_SYSINFO => sys_sysinfo(args[0]),
        SYS_SETPGID | SYS_GETPGID | SYS_SETSID => 0,
        SYS_PRCTL => 0,
        SYS_GETRUSAGE => 0,
        _ => {
            crate::println!("[sys] unimpl {} args={:#x?}", id, args);
            -38
        }
    }
}

fn current_pt_read(buf: usize, len: usize) -> alloc::vec::Vec<u8> {
    let t = crate::task::current();
    let ms = t.memory.lock();
    read_user_bytes(&ms.page_table, VirtAddr(buf), len)
}

fn current_pt_write(buf: usize, data: &[u8]) {
    let t = crate::task::current();
    let ms = t.memory.lock();
    write_user_bytes(&ms.page_table, VirtAddr(buf), data);
}

fn user_cstr(va: usize) -> String {
    use crate::mm::page_table::read_cstr;
    let t = crate::task::current();
    let ms = t.memory.lock();
    read_cstr(&ms.page_table, VirtAddr(va))
}

fn sys_write(fd: i32, buf: usize, len: usize) -> isize {
    let data = current_pt_read(buf, len);
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    file.write(&data)
}

fn sys_read(fd: i32, buf: usize, len: usize) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let mut tmp = alloc::vec![0u8; len];
    let n = file.read(&mut tmp);
    if n > 0 {
        current_pt_write(buf, &tmp[..n as usize]);
    }
    n
}

#[repr(C)]
struct IoVec { base: usize, len: usize }

fn sys_writev(fd: i32, iov: usize, cnt: usize) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let mut total: isize = 0;
    let bytes = current_pt_read(iov, cnt * core::mem::size_of::<IoVec>());
    let vecs: &[IoVec] = unsafe {
        core::slice::from_raw_parts(bytes.as_ptr() as *const IoVec, cnt)
    };
    for v in vecs {
        if v.len == 0 { continue; }
        let data = current_pt_read(v.base, v.len);
        let n = file.write(&data);
        if n < 0 { return if total == 0 { n } else { total }; }
        total += n;
    }
    total
}

fn sys_readv(fd: i32, iov: usize, cnt: usize) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let bytes = current_pt_read(iov, cnt * core::mem::size_of::<IoVec>());
    let vecs: &[IoVec] = unsafe {
        core::slice::from_raw_parts(bytes.as_ptr() as *const IoVec, cnt)
    };
    let mut total: isize = 0;
    for v in vecs {
        if v.len == 0 { continue; }
        let mut tmp = alloc::vec![0u8; v.len];
        let n = file.read(&mut tmp);
        if n <= 0 { return if total == 0 { n } else { total }; }
        current_pt_write(v.base, &tmp[..n as usize]);
        total += n;
        if (n as usize) < v.len { break; }
    }
    total
}

fn sys_close(fd: i32) -> isize {
    let t = crate::task::current();
    if t.files.lock().close(fd) { 0 } else { -9 }
}

fn sys_brk(addr: usize) -> isize {
    let t = crate::task::current();
    let mut brk = t.program_break.lock();
    if addr == 0 { return *brk as isize; }
    if addr < t.heap_base { return *brk as isize; }
    let cur = *brk;
    if addr > cur {
        let mut ms = t.memory.lock();
        let start = VirtAddr(cur).ceil().base().as_usize();
        if start < addr {
            let mut a = MapArea::new(VirtAddr(start), VirtAddr(addr), MapPerm::R | MapPerm::W | MapPerm::U, MapType::Framed);
            a.map(&mut ms.page_table);
            ms.areas.push(a);
        }
    }
    *brk = addr;
    addr as isize
}

#[repr(C)]
struct UtsName {
    sysname: [u8; 65], nodename: [u8; 65], release: [u8; 65],
    version: [u8; 65], machine: [u8; 65], domainname: [u8; 65],
}

fn sys_uname(buf: usize) -> isize {
    let mut u = UtsName {
        sysname: [0; 65], nodename: [0; 65], release: [0; 65],
        version: [0; 65], machine: [0; 65], domainname: [0; 65],
    };
    fn set(dst: &mut [u8], s: &[u8]) { dst[..s.len()].copy_from_slice(s); }
    set(&mut u.sysname, b"Linux");
    set(&mut u.nodename, b"ijg");
    set(&mut u.release, b"5.15.0");
    set(&mut u.version, b"#1 SMP");
    set(&mut u.machine, b"riscv64");
    set(&mut u.domainname, b"(none)");
    let bytes = unsafe {
        core::slice::from_raw_parts(&u as *const _ as *const u8, core::mem::size_of::<UtsName>())
    };
    current_pt_write(buf, bytes);
    0
}

#[repr(C)]
struct TimeSpec { sec: u64, nsec: u64 }

fn sys_clock_gettime(_clk: usize, out: usize) -> isize {
    let ns = crate::timer::now_ns();
    let ts = TimeSpec { sec: ns / 1_000_000_000, nsec: ns % 1_000_000_000 };
    let bytes = unsafe { core::slice::from_raw_parts(&ts as *const _ as *const u8, 16) };
    current_pt_write(out, bytes);
    0
}

fn sys_gettimeofday(out: usize) -> isize {
    let ns = crate::timer::now_ns();
    let ts = TimeSpec { sec: ns / 1_000_000_000, nsec: (ns % 1_000_000_000) / 1000 };
    let bytes = unsafe { core::slice::from_raw_parts(&ts as *const _ as *const u8, 16) };
    current_pt_write(out, bytes);
    0
}

// --- VFS syscalls ------------------------------------------------------

const AT_FDCWD: i32 = -100;

fn resolve_path(dirfd: i32, path: &str) -> String {
    if path.starts_with('/') { return path.into(); }
    if dirfd == AT_FDCWD {
        let t = crate::task::current();
        let cwd = t.cwd.lock().clone();
        let mut p = cwd;
        if !p.ends_with('/') { p.push('/'); }
        p.push_str(path);
        return p;
    }
    // TODO dirfd-relative
    path.into()
}

fn sys_openat(dirfd: i32, path: usize, _flags: u32, _mode: u32) -> isize {
    let p = user_cstr(path);
    let full = resolve_path(dirfd, &p);
    let result = crate::fs::VFS.open(&full);
    let Some(file) = result else {
        return -2;
    };
    let t = crate::task::current();
    let r = t.files.lock().alloc(file).map(|x| x as isize).unwrap_or(-24);
    r
}

#[repr(C)]
struct KStat {
    dev: u64, ino: u64, mode: u32, nlink: u32,
    uid: u32, gid: u32, rdev: u64, _pad: u64,
    size: i64, blksize: u32, _pad2: u32, blocks: i64,
    atime: i64, atime_ns: u64,
    mtime: i64, mtime_ns: u64,
    ctime: i64, ctime_ns: u64,
    _pad3: [u32; 2],
}

fn stat_file(path: &str, out: usize) -> isize {
    if !crate::fs::VFS.exists(path) { return -2; }
    let is_dir = crate::fs::VFS.is_dir(path).unwrap_or(false);
    let size = crate::fs::VFS.size(path).unwrap_or(0) as i64;
    let mode = if is_dir { 0o040755 } else { 0o100644 };
    let st = KStat {
        dev: 1, ino: 1, mode, nlink: 1,
        uid: 0, gid: 0, rdev: 0, _pad: 0,
        size, blksize: 4096, _pad2: 0, blocks: (size + 511) / 512,
        atime: 0, atime_ns: 0, mtime: 0, mtime_ns: 0, ctime: 0, ctime_ns: 0,
        _pad3: [0; 2],
    };
    let bytes = unsafe { core::slice::from_raw_parts(&st as *const _ as *const u8, core::mem::size_of::<KStat>()) };
    current_pt_write(out, bytes);
    0
}

fn sys_fstat(fd: i32, out: usize) -> isize {
    let t = crate::task::current();
    let Some(file) = t.files.lock().get(fd) else { return -9 };
    let size = file.size() as i64;
    let st = KStat {
        dev: 1, ino: file.inode_id(), mode: 0o100644, nlink: 1,
        uid: 0, gid: 0, rdev: 0, _pad: 0,
        size, blksize: 4096, _pad2: 0, blocks: (size + 511) / 512,
        atime: 0, atime_ns: 0, mtime: 0, mtime_ns: 0, ctime: 0, ctime_ns: 0,
        _pad3: [0; 2],
    };
    let bytes = unsafe { core::slice::from_raw_parts(&st as *const _ as *const u8, core::mem::size_of::<KStat>()) };
    current_pt_write(out, bytes);
    0
}

fn sys_newfstatat(dirfd: i32, path: usize, out: usize, _flags: i32) -> isize {
    let p = user_cstr(path);
    if p.is_empty() && dirfd >= 0 {
        return sys_fstat(dirfd, out);
    }
    let full = resolve_path(dirfd, &p);
    stat_file(&full, out)
}

fn sys_lseek(fd: i32, off: isize, whence: u32) -> isize {
    let t = crate::task::current();
    let Some(file) = t.files.lock().get(fd) else { return -9 };
    file.seek(off, whence)
}

fn sys_pread(fd: i32, buf: usize, len: usize, off: u64) -> isize {
    let t = crate::task::current();
    let Some(file) = t.files.lock().get(fd) else { return -9 };
    let mut tmp = alloc::vec![0u8; len];
    let n = file.pread(&mut tmp, off);
    if n > 0 { current_pt_write(buf, &tmp[..n as usize]); }
    n
}

fn sys_readlinkat(_dirfd: i32, path: usize, buf: usize, sz: usize) -> isize {
    let p = user_cstr(path);
    // /proc/self/exe could be asked — return a best-effort
    if p == "/proc/self/exe" {
        let s = b"/usr/sbin/nginx";
        let n = core::cmp::min(s.len(), sz);
        current_pt_write(buf, &s[..n]);
        return n as isize;
    }
    -22
}

fn sys_getcwd(buf: usize, sz: usize) -> isize {
    let t = crate::task::current();
    let cwd = t.cwd.lock().clone();
    let bytes = cwd.as_bytes();
    if bytes.len() + 1 > sz { return -34; }
    current_pt_write(buf, bytes);
    current_pt_write(buf + bytes.len(), &[0u8]);
    buf as isize
}

fn sys_faccessat(_dirfd: i32, path: usize, _mode: i32, _flags: i32) -> isize {
    let p = user_cstr(path);
    if crate::fs::VFS.exists(&p) { 0 } else { -2 }
}

// --- mmap / mprotect ---------------------------------------------------

const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const PROT_EXEC: i32 = 4;
const MAP_PRIVATE: i32 = 0x02;
const MAP_ANONYMOUS: i32 = 0x20;
const MAP_FIXED: i32 = 0x10;

fn prot_to_perm(prot: i32) -> MapPerm {
    let mut p = MapPerm::U;
    if prot & PROT_READ != 0 { p |= MapPerm::R; }
    if prot & PROT_WRITE != 0 { p |= MapPerm::W; }
    if prot & PROT_EXEC != 0 { p |= MapPerm::X; }
    p
}

fn sys_mmap(addr: usize, length: usize, prot: i32, flags: i32, fd: i32, offset: u64) -> isize {
    if length == 0 { return -22; }
    let len = (length + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let perm = prot_to_perm(prot);

    let t = crate::task::current();
    let va = if flags & MAP_FIXED != 0 && addr != 0 {
        addr & !(PAGE_SIZE - 1)
    } else {
        let mut top = t.mmap_top.lock();
        *top -= len;
        *top &= !(PAGE_SIZE - 1);
        *top
    };

    {
        let mut ms = t.memory.lock();
        if flags & MAP_FIXED != 0 {
            let svpn = VirtAddr(va).floor();
            let evpn = VirtAddr(va + len).ceil();
            for vpn in svpn.0..evpn.0 {
                if ms.page_table.find_pte(VirtPageNum(vpn)).is_some() {
                    ms.page_table.unmap(VirtPageNum(vpn));
                }
            }
        }
        let eff_perm = perm | MapPerm::R | MapPerm::W;
        let mut area = MapArea::new(VirtAddr(va), VirtAddr(va + len), eff_perm, MapType::Framed);
        area.map(&mut ms.page_table);
        ms.areas.push(area);
    }

    if flags & MAP_ANONYMOUS == 0 && fd >= 0 {
        let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
        let mut tmp = alloc::vec![0u8; len];
        let n = file.pread(&mut tmp, offset);
        if n > 0 {
            current_pt_write(va, &tmp[..n as usize]);
        }
    }
    va as isize
}

fn sys_munmap(_addr: usize, _len: usize) -> isize { 0 }
fn sys_mprotect(_addr: usize, _len: usize, _prot: i32) -> isize { 0 }

fn sys_getrandom(buf: usize, len: usize, _flags: u32) -> isize {
    let mut data = alloc::vec![0u8; len];
    // poor man's PRNG based on timer
    let mut seed = crate::timer::now_ns();
    for b in data.iter_mut() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (seed >> 33) as u8;
    }
    current_pt_write(buf, &data);
    len as isize
}

fn sys_prlimit64(_pid: usize, _res: i32, _new: usize, old: usize) -> isize {
    if old != 0 {
        #[repr(C)]
        struct Rlim { cur: u64, max: u64 }
        let r = Rlim { cur: 1 << 20, max: 1 << 20 };
        let bytes = unsafe { core::slice::from_raw_parts(&r as *const _ as *const u8, 16) };
        current_pt_write(old, bytes);
    }
    0
}

fn sys_ioctl(_fd: i32, _req: u32, _arg: usize) -> isize { 0 }

const F_DUPFD: u32 = 0;
const F_GETFD: u32 = 1;
const F_SETFD: u32 = 2;
const F_GETFL: u32 = 3;
const F_SETFL: u32 = 4;
const F_DUPFD_CLOEXEC: u32 = 1030;

fn sys_fcntl(fd: i32, cmd: u32, _arg: usize) -> isize {
    let t = crate::task::current();
    match cmd {
        F_DUPFD | F_DUPFD_CLOEXEC => {
            let f = match t.files.lock().get(fd) { Some(x) => x, None => return -9 };
            let r = t.files.lock().alloc(f).map(|x| x as isize).unwrap_or(-24);
            r
        }
        F_GETFL => 0,
        F_SETFL => 0,
        F_GETFD => 0,
        F_SETFD => 0,
        _ => 0,
    }
}

fn sys_dup(fd: i32) -> isize {
    let t = crate::task::current();
    let f = match t.files.lock().get(fd) { Some(x) => x, None => return -9 };
    let r = t.files.lock().alloc(f).map(|x| x as isize).unwrap_or(-24);
    r
}

fn sys_dup3(old: i32, new: i32, _flags: u32) -> isize {
    let t = crate::task::current();
    let f = match t.files.lock().get(old) { Some(x) => x, None => return -9 };
    let mut ft = t.files.lock();
    while ft.files.len() <= new as usize { ft.files.push(None); }
    ft.files[new as usize] = Some(f);
    new as isize
}

fn sys_pipe2(_buf: usize, _flags: u32) -> isize { -38 }
fn sys_eventfd2(_initval: u32, _flags: u32) -> isize { -38 }
fn sys_epoll_create1(_flags: u32) -> isize { -38 }

#[repr(C)]
struct SysInfo { uptime: i64, loads: [u64; 3], totalram: u64, freeram: u64,
                 sharedram: u64, bufferram: u64, totalswap: u64, freeswap: u64,
                 procs: u16, _pad: u16, totalhigh: u64, freehigh: u64, mem_unit: u32, _pad2: [u8; 8] }

fn sys_sysinfo(out: usize) -> isize {
    let si = SysInfo {
        uptime: crate::timer::now_sec() as i64,
        loads: [0; 3],
        totalram: 512 * 1024 * 1024, freeram: 256 * 1024 * 1024,
        sharedram: 0, bufferram: 0, totalswap: 0, freeswap: 0,
        procs: 1, _pad: 0, totalhigh: 0, freehigh: 0, mem_unit: 1, _pad2: [0; 8],
    };
    let bytes = unsafe { core::slice::from_raw_parts(&si as *const _ as *const u8, core::mem::size_of::<SysInfo>()) };
    current_pt_write(out, bytes);
    0
}

// --- sockets ----------------------------------------------------------

fn sys_socket(domain: i32, _type: i32, _proto: i32) -> isize {
    if domain != 2 { return -97; }
    let t = crate::task::current();
    let s = crate::fs::SocketFile::new();
    let fd = match t.files.lock().alloc(s as Arc<dyn crate::fs::File>) { Some(f) => f, None => return -24 };
    fd as isize
}

#[repr(C)]
struct SockAddrIn { family: u16, port: u16, addr: u32, zero: [u8; 8] }

fn sys_bind(fd: i32, addr: usize, _len: u32) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let Some(sf) = file.as_socket() else { return -88 };
    let bytes = current_pt_read(addr, core::mem::size_of::<SockAddrIn>());
    let sa: SockAddrIn = unsafe { core::ptr::read(bytes.as_ptr() as *const SockAddrIn) };
    let port = u16::from_be(sa.port);
    *sf.state.lock() = crate::fs::SocketState::Listening { port };
    0
}

fn sys_listen(fd: i32, _backlog: i32) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let Some(sf) = file.as_socket() else { return -88 };
    let port = match *sf.state.lock() {
        crate::fs::SocketState::Listening { port } => port,
        _ => return -22,
    };
    let sock_guard = sf.sock.lock();
    let Some(sock) = sock_guard.as_ref() else { return -22 };
    if crate::net::tcp_bind_listen(sock, port) { 0 } else { -98 }
}

fn sys_accept(fd: i32, _addr: usize, _alen: usize, _flags: i32) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let Some(sf) = file.as_socket() else { return -88 };
    loop {
        crate::net::poll();
        let is_active = {
            let g = sf.sock.lock();
            if let Some(s) = g.as_ref() { crate::net::tcp_is_active(s) } else { false }
        };
        if is_active { break; }
        unsafe { riscv::asm::wfi(); }
    }
    let port = match *sf.state.lock() {
        crate::fs::SocketState::Listening { port } => port,
        _ => return -22,
    };
    let taken = {
        let mut g = sf.sock.lock();
        let taken = g.take();
        let new_sock = crate::net::tcp_open();
        if let Some(ref s) = new_sock { crate::net::tcp_bind_listen(s, port); }
        *g = new_sock;
        taken
    };
    let Some(conn) = taken else { return -22 };
    let conn_file = Arc::new(crate::fs::SocketFile {
        sock: spin::Mutex::new(Some(conn)),
        state: spin::Mutex::new(crate::fs::SocketState::Connected),
        nonblocking: core::sync::atomic::AtomicBool::new(false),
    });
    let new_fd = match t.files.lock().alloc(conn_file as Arc<dyn crate::fs::File>) {
        Some(f) => f, None => return -24,
    };
    new_fd as isize
}

fn sys_shutdown(fd: i32) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let Some(sf) = file.as_socket() else { return -88 };
    let g = sf.sock.lock();
    if let Some(s) = g.as_ref() { crate::net::tcp_close(s); }
    0
}
