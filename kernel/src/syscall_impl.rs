use crate::syscall::*;
use crate::trap::TrapContext;
use crate::mm::page_table::{read_cstr, read_user_bytes, write_user_bytes};
use crate::mm::address::VirtAddr;

pub fn dispatch(id: usize, args: [usize; 6], cx: &mut TrapContext) -> isize {
    match id {
        SYS_WRITE => sys_write(args[0] as i32, args[1], args[2]),
        SYS_READ => sys_read(args[0] as i32, args[1], args[2]),
        SYS_WRITEV => sys_writev(args[0] as i32, args[1], args[2]),
        SYS_EXIT | SYS_EXIT_GROUP => crate::task::exit_current(args[0] as i32),
        SYS_GETPID => crate::task::current().pid as isize,
        SYS_GETUID | SYS_GETEUID | SYS_GETGID | SYS_GETEGID => 0,
        SYS_GETTID => crate::task::current().pid as isize,
        SYS_SCHED_YIELD => { crate::task::yield_current(); 0 }
        SYS_BRK => sys_brk(args[0]),
        SYS_SET_TID_ADDRESS | SYS_SET_ROBUST_LIST => 0,
        SYS_RT_SIGACTION | SYS_RT_SIGPROCMASK => 0,
        SYS_UNAME => sys_uname(args[0]),
        SYS_CLOCK_GETTIME => sys_clock_gettime(args[0], args[1]),
        SYS_GETTIMEOFDAY => sys_gettimeofday(args[0]),
        _ => {
            crate::println!("[kernel] unimpl syscall {}", id);
            -38
        }
    }
}

fn sys_write(fd: i32, buf: usize, len: usize) -> isize {
    let t = crate::task::current();
    let pt = &t.memory.lock().page_table as *const _;
    // SAFETY: we don't free the page table concurrently
    let pt = unsafe { &*pt };
    let data = read_user_bytes(pt, VirtAddr(buf), len);
    let file = match t.files.lock().get(fd) {
        Some(f) => f, None => return -9,
    };
    file.write(&data)
}

fn sys_read(fd: i32, buf: usize, len: usize) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) {
        Some(f) => f, None => return -9,
    };
    let mut tmp = alloc::vec![0u8; len];
    let n = file.read(&mut tmp);
    if n > 0 {
        let pt_guard = t.memory.lock();
        let pt = &pt_guard.page_table;
        write_user_bytes(pt, VirtAddr(buf), &tmp[..n as usize]);
    }
    n
}

#[repr(C)]
struct IoVec { base: usize, len: usize }

fn sys_writev(fd: i32, iov: usize, cnt: usize) -> isize {
    let t = crate::task::current();
    let pt_guard = t.memory.lock();
    let pt = &pt_guard.page_table;
    let mut total: isize = 0;
    let bytes = read_user_bytes(pt, VirtAddr(iov), cnt * core::mem::size_of::<IoVec>());
    let vecs: &[IoVec] = unsafe {
        core::slice::from_raw_parts(bytes.as_ptr() as *const IoVec, cnt)
    };
    let file = match t.files.lock().get(fd) {
        Some(f) => f, None => return -9,
    };
    for v in vecs {
        if v.len == 0 { continue; }
        let data = read_user_bytes(pt, VirtAddr(v.base), v.len);
        let n = file.write(&data);
        if n < 0 { return if total == 0 { n } else { total }; }
        total += n;
    }
    total
}

fn sys_brk(addr: usize) -> isize {
    let t = crate::task::current();
    let mut brk = t.program_break.lock();
    if addr == 0 { return *brk as isize; }
    if addr < t.heap_base { return *brk as isize; }
    // grow heap via framed mapping
    use crate::mm::memory_set::{MapArea, MapPerm, MapType};
    let cur = *brk;
    if addr > cur {
        let mut ms = t.memory.lock();
        let mut area = MapArea::new(VirtAddr(cur), VirtAddr(addr), MapPerm::R | MapPerm::W | MapPerm::U, MapType::Framed);
        // align start to page
        let start = VirtAddr(cur).ceil().base().as_usize();
        if start < addr {
            let mut a = MapArea::new(VirtAddr(start), VirtAddr(addr), MapPerm::R | MapPerm::W | MapPerm::U, MapType::Framed);
            a.map(&mut ms.page_table);
            ms.areas.push(a);
        }
        let _ = area; // was a duplicate; keep lint happy
    }
    *brk = addr;
    addr as isize
}

#[repr(C)]
struct UtsName {
    sysname: [u8; 65],
    nodename: [u8; 65],
    release: [u8; 65],
    version: [u8; 65],
    machine: [u8; 65],
    domainname: [u8; 65],
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
    let t = crate::task::current();
    let ms = t.memory.lock();
    let bytes = unsafe {
        core::slice::from_raw_parts(&u as *const _ as *const u8, core::mem::size_of::<UtsName>())
    };
    write_user_bytes(&ms.page_table, VirtAddr(buf), bytes);
    0
}

#[repr(C)]
struct TimeSpec { sec: u64, nsec: u64 }

fn sys_clock_gettime(_clk: usize, out: usize) -> isize {
    let ns = crate::timer::now_ns();
    let ts = TimeSpec { sec: ns / 1_000_000_000, nsec: ns % 1_000_000_000 };
    let t = crate::task::current();
    let ms = t.memory.lock();
    let bytes = unsafe {
        core::slice::from_raw_parts(&ts as *const _ as *const u8, core::mem::size_of::<TimeSpec>())
    };
    write_user_bytes(&ms.page_table, VirtAddr(out), bytes);
    0
}

fn sys_gettimeofday(out: usize) -> isize {
    let ns = crate::timer::now_ns();
    let ts = TimeSpec { sec: ns / 1_000_000_000, nsec: (ns % 1_000_000_000) / 1000 };
    let t = crate::task::current();
    let ms = t.memory.lock();
    let bytes = unsafe {
        core::slice::from_raw_parts(&ts as *const _ as *const u8, 16)
    };
    write_user_bytes(&ms.page_table, VirtAddr(out), bytes);
    0
}
