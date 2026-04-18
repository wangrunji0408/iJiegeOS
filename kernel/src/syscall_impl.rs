use crate::syscall::*;
use crate::trap::TrapContext;
use crate::mm::page_table::{read_user_bytes, write_user_bytes};
use crate::mm::address::VirtAddr;
use alloc::sync::Arc;

pub fn dispatch(id: usize, args: [usize; 6], _cx: &mut TrapContext) -> isize {
    match id {
        SYS_WRITE => sys_write(args[0] as i32, args[1], args[2]),
        SYS_READ => sys_read(args[0] as i32, args[1], args[2]),
        SYS_WRITEV => sys_writev(args[0] as i32, args[1], args[2]),
        SYS_CLOSE => sys_close(args[0] as i32),
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
        SYS_SOCKET => sys_socket(args[0] as i32, args[1] as i32, args[2] as i32),
        SYS_BIND => sys_bind(args[0] as i32, args[1], args[2] as u32),
        SYS_LISTEN => sys_listen(args[0] as i32, args[1] as i32),
        SYS_ACCEPT | SYS_ACCEPT4 => sys_accept(args[0] as i32, args[1], args[2], args[3] as i32),
        SYS_SETSOCKOPT | SYS_GETSOCKOPT => 0,
        SYS_SHUTDOWN => sys_shutdown(args[0] as i32),
        _ => {
            crate::println!("[kernel] unimpl syscall {} args={:?}", id, args);
            -38
        }
    }
}

fn sys_write(fd: i32, buf: usize, len: usize) -> isize {
    let t = crate::task::current();
    let data = {
        let ms = t.memory.lock();
        read_user_bytes(&ms.page_table, VirtAddr(buf), len)
    };
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    file.write(&data)
}

fn sys_read(fd: i32, buf: usize, len: usize) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let mut tmp = alloc::vec![0u8; len];
    let n = file.read(&mut tmp);
    if n > 0 {
        let ms = t.memory.lock();
        write_user_bytes(&ms.page_table, VirtAddr(buf), &tmp[..n as usize]);
    }
    n
}

#[repr(C)]
struct IoVec { base: usize, len: usize }

fn sys_writev(fd: i32, iov: usize, cnt: usize) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let mut total: isize = 0;
    let bytes = {
        let ms = t.memory.lock();
        read_user_bytes(&ms.page_table, VirtAddr(iov), cnt * core::mem::size_of::<IoVec>())
    };
    let vecs: &[IoVec] = unsafe {
        core::slice::from_raw_parts(bytes.as_ptr() as *const IoVec, cnt)
    };
    for v in vecs {
        if v.len == 0 { continue; }
        let data = {
            let ms = t.memory.lock();
            read_user_bytes(&ms.page_table, VirtAddr(v.base), v.len)
        };
        let n = file.write(&data);
        if n < 0 { return if total == 0 { n } else { total }; }
        total += n;
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
    use crate::mm::memory_set::{MapArea, MapPerm, MapType};
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
        core::slice::from_raw_parts(&ts as *const _ as *const u8, 16)
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

// --- socket -------------------------------------------------------------

fn sys_socket(domain: i32, _type: i32, _proto: i32) -> isize {
    // AF_INET = 2
    if domain != 2 { return -97; } // EAFNOSUPPORT
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
    let bytes = {
        let ms = t.memory.lock();
        read_user_bytes(&ms.page_table, VirtAddr(addr), core::mem::size_of::<SockAddrIn>())
    };
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
    if crate::net::tcp_bind_listen(sock, port) { 0 } else { -98 } // EADDRINUSE
}

fn sys_accept(fd: i32, _addr: usize, _alen: usize, _flags: i32) -> isize {
    let t = crate::task::current();
    let file = match t.files.lock().get(fd) { Some(f) => f, None => return -9 };
    let Some(sf) = file.as_socket() else { return -88 };
    // Block-wait for incoming connection on our listening socket
    loop {
        crate::net::poll();
        let is_active = {
            let g = sf.sock.lock();
            if let Some(s) = g.as_ref() { crate::net::tcp_is_active(s) } else { false }
        };
        if is_active { break; }
        unsafe { riscv::asm::wfi(); }
    }
    // "Hand off" current socket into a new fd; create a fresh listening socket
    let port = match *sf.state.lock() {
        crate::fs::SocketState::Listening { port } => port,
        _ => return -22,
    };
    let taken = {
        let mut g = sf.sock.lock();
        let taken = g.take();
        // Replace with a fresh listening socket
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
