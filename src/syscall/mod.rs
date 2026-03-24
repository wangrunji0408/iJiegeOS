use crate::trap::TrapContext;
use crate::config::PAGE_SIZE;

// Linux RISC-V system call numbers
mod nr {
    pub const GETCWD: usize = 17;
    pub const DUP: usize = 23;
    pub const DUP3: usize = 24;
    pub const FCNTL: usize = 25;
    pub const IOCTL: usize = 29;
    pub const MKDIRAT: usize = 34;
    pub const UNLINKAT: usize = 35;
    pub const LINKAT: usize = 37;
    pub const UMOUNT2: usize = 39;
    pub const MOUNT: usize = 40;
    pub const FACCESSAT: usize = 48;
    pub const CHDIR: usize = 49;
    pub const OPENAT: usize = 56;
    pub const CLOSE: usize = 57;
    pub const PIPE2: usize = 59;
    pub const GETDENTS64: usize = 61;
    pub const LSEEK: usize = 62;
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const READV: usize = 65;
    pub const WRITEV: usize = 66;
    pub const PREAD64: usize = 67;
    pub const SENDFILE: usize = 71;
    pub const PSELECT6: usize = 72;
    pub const PPOLL: usize = 73;
    pub const READLINKAT: usize = 78;
    pub const FSTATAT: usize = 79;
    pub const FSTAT: usize = 80;
    pub const FSYNC: usize = 82;
    pub const UTIMENSAT: usize = 88;
    pub const EXIT: usize = 93;
    pub const EXIT_GROUP: usize = 94;
    pub const SET_TID_ADDRESS: usize = 96;
    pub const FUTEX: usize = 98;
    pub const NANOSLEEP: usize = 101;
    pub const SETITIMER: usize = 103;
    pub const CLOCK_GETTIME: usize = 113;
    pub const SYSLOG: usize = 116;
    pub const SCHED_YIELD: usize = 124;
    pub const KILL: usize = 129;
    pub const SIGACTION: usize = 134;
    pub const SIGPROCMASK: usize = 135;
    pub const SIGRETURN: usize = 139;
    pub const TIMES: usize = 153;
    pub const UNAME: usize = 160;
    pub const GETRUSAGE: usize = 165;
    pub const GETTIMEOFDAY: usize = 169;
    pub const GETPID: usize = 172;
    pub const GETPPID: usize = 173;
    pub const GETUID: usize = 174;
    pub const GETEUID: usize = 175;
    pub const GETGID: usize = 176;
    pub const GETEGID: usize = 177;
    pub const GETTID: usize = 178;
    pub const SOCKET: usize = 198;
    pub const SOCKETPAIR: usize = 199;
    pub const BIND: usize = 200;
    pub const LISTEN: usize = 201;
    pub const ACCEPT: usize = 202;
    pub const CONNECT: usize = 203;
    pub const GETSOCKNAME: usize = 204;
    pub const GETPEERNAME: usize = 205;
    pub const SENDTO: usize = 206;
    pub const RECVFROM: usize = 207;
    pub const SETSOCKOPT: usize = 208;
    pub const GETSOCKOPT: usize = 209;
    pub const SHUTDOWN: usize = 210;
    pub const BRK: usize = 214;
    pub const MUNMAP: usize = 215;
    pub const CLONE: usize = 220;
    pub const EXECVE: usize = 221;
    pub const MMAP: usize = 222;
    pub const MPROTECT: usize = 226;
    pub const WAIT4: usize = 260;
    pub const PRLIMIT64: usize = 261;
    pub const GETRANDOM: usize = 278;
    pub const RSEQ: usize = 293;
    pub const EPOLL_CREATE1: usize = 20;
    pub const EPOLL_CTL: usize = 21;
    pub const EPOLL_PWAIT: usize = 22;
    pub const EVENTFD2: usize = 19;
    pub const ACCEPT4: usize = 242;
    pub const POLL: usize = 1079; // Not standard on RISC-V, epoll is used instead
    pub const CLOCK_GETRES: usize = 114;
    pub const STATX: usize = 291;
    pub const SET_ROBUST_LIST: usize = 99;
}

pub fn syscall(id: usize, args: [usize; 6], cx: &mut TrapContext) -> isize {
    match id {
        nr::WRITE => sys_write(args[0], args[1], args[2]),
        nr::READ => sys_read(args[0], args[1], args[2]),
        nr::WRITEV => sys_writev(args[0], args[1], args[2]),
        nr::READV => sys_readv(args[0], args[1], args[2]),
        nr::CLOSE => sys_close(args[0]),
        nr::EXIT | nr::EXIT_GROUP => sys_exit(args[0] as i32),
        nr::BRK => sys_brk(args[0]),
        nr::MMAP => sys_mmap(args[0], args[1], args[2], args[3], args[4] as i32, args[5]),
        nr::MUNMAP => sys_munmap(args[0], args[1]),
        nr::MPROTECT => sys_mprotect(args[0], args[1], args[2]),
        nr::GETPID => sys_getpid(),
        nr::GETPPID => sys_getppid(),
        nr::GETTID => sys_gettid(),
        nr::GETUID | nr::GETEUID | nr::GETGID | nr::GETEGID => 0,
        nr::SET_TID_ADDRESS => sys_set_tid_address(args[0]),
        nr::UNAME => sys_uname(args[0]),
        nr::CLOCK_GETTIME => sys_clock_gettime(args[0], args[1]),
        nr::GETTIMEOFDAY => sys_gettimeofday(args[0], args[1]),
        nr::NANOSLEEP => sys_nanosleep(args[0], args[1]),
        nr::SIGACTION => sys_sigaction(args[0], args[1], args[2]),
        nr::SIGPROCMASK => sys_sigprocmask(args[0], args[1], args[2], args[3]),
        nr::OPENAT => sys_openat(args[0] as i32, args[1], args[2] as i32, args[3] as u32),
        nr::FSTAT => sys_fstat(args[0], args[1]),
        nr::FSTATAT => sys_fstatat(args[0] as i32, args[1], args[2], args[3] as i32),
        nr::STATX => sys_statx(args[0] as i32, args[1], args[2] as i32, args[3] as u32, args[4]),
        nr::FCNTL => sys_fcntl(args[0], args[1], args[2]),
        nr::IOCTL => sys_ioctl(args[0], args[1], args[2]),
        nr::GETCWD => sys_getcwd(args[0], args[1]),
        nr::DUP => sys_dup(args[0]),
        nr::DUP3 => sys_dup3(args[0], args[1], args[2] as i32),
        nr::PIPE2 => sys_pipe2(args[0], args[1] as i32),
        nr::LSEEK => sys_lseek(args[0], args[1] as isize, args[2] as i32),
        nr::PREAD64 => sys_pread64(args[0], args[1], args[2], args[3]),
        nr::PRLIMIT64 => sys_prlimit64(args[0], args[1], args[2], args[3]),
        nr::GETRANDOM => sys_getrandom(args[0], args[1], args[2]),
        nr::SCHED_YIELD => { crate::process::schedule(); 0 },
        nr::CLONE => sys_clone(args[0], args[1], args[2], args[3], args[4]),
        nr::EXECVE => sys_execve(args[0], args[1], args[2]),
        nr::WAIT4 => sys_wait4(args[0] as isize, args[1], args[2] as i32, args[3]),
        nr::KILL => sys_kill(args[0], args[1]),
        nr::SOCKET => sys_socket(args[0] as i32, args[1] as i32, args[2] as i32),
        nr::BIND => sys_bind(args[0], args[1], args[2]),
        nr::LISTEN => sys_listen(args[0], args[1] as i32),
        nr::ACCEPT | nr::ACCEPT4 => sys_accept(args[0], args[1], args[2]),
        nr::CONNECT => sys_connect(args[0], args[1], args[2]),
        nr::SETSOCKOPT => sys_setsockopt(args[0], args[1] as i32, args[2] as i32, args[3], args[4]),
        nr::GETSOCKOPT => sys_getsockopt(args[0], args[1] as i32, args[2] as i32, args[3], args[4]),
        nr::GETSOCKNAME => sys_getsockname(args[0], args[1], args[2]),
        nr::GETPEERNAME => sys_getpeername(args[0], args[1], args[2]),
        nr::SENDTO => sys_sendto(args[0], args[1], args[2], args[3] as i32, args[4], args[5]),
        nr::RECVFROM => sys_recvfrom(args[0], args[1], args[2], args[3] as i32, args[4], args[5]),
        nr::SHUTDOWN => sys_shutdown_sock(args[0], args[1] as i32),
        nr::EPOLL_CREATE1 => sys_epoll_create1(args[0] as i32),
        nr::EPOLL_CTL => sys_epoll_ctl(args[0], args[1] as i32, args[2], args[3]),
        nr::EPOLL_PWAIT => sys_epoll_pwait(args[0], args[1], args[2] as i32, args[3] as i32, args[4]),
        nr::EVENTFD2 => sys_eventfd2(args[0] as u32, args[1] as i32),
        nr::PPOLL => sys_ppoll(args[0], args[1], args[2], args[3]),
        nr::FUTEX => 0, // stub
        nr::RSEQ => -38isize, // ENOSYS
        nr::SET_ROBUST_LIST => 0, // stub
        nr::TIMES => sys_times(args[0]),
        nr::GETRUSAGE => sys_getrusage(args[0] as i32, args[1]),
        nr::SETITIMER => sys_setitimer(args[0] as i32, args[1], args[2]),
        nr::SYSLOG => 0, // stub
        nr::SENDFILE => sys_sendfile(args[0], args[1], args[2], args[3]),
        nr::READLINKAT => sys_readlinkat(args[0] as i32, args[1], args[2], args[3]),
        nr::FACCESSAT => sys_faccessat(args[0] as i32, args[1], args[2] as i32),
        nr::MKDIRAT => sys_mkdirat(args[0] as i32, args[1], args[2] as u32),
        nr::UNLINKAT => sys_unlinkat(args[0] as i32, args[1], args[2] as i32),
        nr::SOCKETPAIR => sys_socketpair(args[0] as i32, args[1] as i32, args[2] as i32, args[3]),
        nr::CLOCK_GETRES => sys_clock_getres(args[0], args[1]),
        nr::UTIMENSAT => 0, // stub
        nr::FSYNC => 0, // stub
        nr::GETDENTS64 => sys_getdents64(args[0], args[1], args[2]),
        _ => {
            println!("[syscall] Unimplemented syscall: {} (args: {:#x}, {:#x}, {:#x})", id, args[0], args[1], args[2]);
            -38 // ENOSYS
        }
    }
}

fn read_user_bytes(va: usize, len: usize) -> alloc::vec::Vec<u8> {
    let proc = crate::process::current_process();
    let p = proc.lock();
    let token = p.token();
    drop(p);
    let pt = crate::mm::PageTable::from_token(token);
    let mut result = alloc::vec![0u8; len];
    for i in 0..len {
        let pa = pt.translate_va(crate::mm::VirtAddr(va + i));
        if let Some(pa) = pa {
            result[i] = unsafe { *(pa.0 as *const u8) };
        }
    }
    result
}

fn write_user_data(va: usize, data: &[u8]) {
    let proc = crate::process::current_process();
    let p = proc.lock();
    let token = p.token();
    drop(p);
    let pt = crate::mm::PageTable::from_token(token);
    for (i, &byte) in data.iter().enumerate() {
        let pa = pt.translate_va(crate::mm::VirtAddr(va + i));
        if let Some(pa) = pa {
            unsafe { *(pa.0 as *mut u8) = byte; }
        }
    }
}

fn read_user_cstr(va: usize) -> alloc::string::String {
    let proc = crate::process::current_process();
    let p = proc.lock();
    let token = p.token();
    drop(p);
    crate::mm::translated_str(token, va as *const u8)
}

// ==================== Syscall implementations ====================

fn sys_write(fd: usize, buf_ptr: usize, len: usize) -> isize {
    let data = read_user_bytes(buf_ptr, len);
    let proc = crate::process::current_process();
    let mut p = proc.lock();
    if let Some(fd_obj) = p.get_fd(fd) {
        drop(p);
        fd_obj.lock().write(&data)
    } else {
        -9 // EBADF
    }
}

fn sys_read(fd: usize, buf_ptr: usize, len: usize) -> isize {
    let proc = crate::process::current_process();
    let mut p = proc.lock();
    if let Some(fd_obj) = p.get_fd(fd) {
        drop(p);
        let mut buf = alloc::vec![0u8; len];
        let ret = fd_obj.lock().read(&mut buf);
        if ret > 0 {
            write_user_data(buf_ptr, &buf[..ret as usize]);
        }
        ret
    } else {
        -9 // EBADF
    }
}

fn sys_writev(fd: usize, iov_ptr: usize, iovcnt: usize) -> isize {
    let mut total = 0isize;
    for i in 0..iovcnt {
        let iov_base_bytes = read_user_bytes(iov_ptr + i * 16, 8);
        let iov_len_bytes = read_user_bytes(iov_ptr + i * 16 + 8, 8);
        let iov_base = usize::from_le_bytes(iov_base_bytes.try_into().unwrap());
        let iov_len = usize::from_le_bytes(iov_len_bytes.try_into().unwrap());
        if iov_len > 0 {
            let ret = sys_write(fd, iov_base, iov_len);
            if ret < 0 {
                return if total > 0 { total } else { ret };
            }
            total += ret;
        }
    }
    total
}

fn sys_readv(fd: usize, iov_ptr: usize, iovcnt: usize) -> isize {
    let mut total = 0isize;
    for i in 0..iovcnt {
        let iov_base_bytes = read_user_bytes(iov_ptr + i * 16, 8);
        let iov_len_bytes = read_user_bytes(iov_ptr + i * 16 + 8, 8);
        let iov_base = usize::from_le_bytes(iov_base_bytes.try_into().unwrap());
        let iov_len = usize::from_le_bytes(iov_len_bytes.try_into().unwrap());
        if iov_len > 0 {
            let ret = sys_read(fd, iov_base, iov_len);
            if ret < 0 {
                return if total > 0 { total } else { ret };
            }
            total += ret;
            if (ret as usize) < iov_len {
                break;
            }
        }
    }
    total
}

fn sys_close(fd: usize) -> isize {
    let proc = crate::process::current_process();
    let mut p = proc.lock();
    if p.close_fd(fd) { 0 } else { -9 }
}

fn sys_exit(code: i32) -> isize {
    crate::process::exit_current(code);
}

fn sys_brk(addr: usize) -> isize {
    let proc = crate::process::current_process();
    let mut p = proc.lock();
    if addr == 0 {
        return p.brk as isize;
    }
    if addr < p.brk_start {
        return p.brk as isize;
    }
    // Grow or shrink the heap
    let old_brk = p.brk;
    let new_brk = addr;
    if new_brk > old_brk {
        // Allocate new pages
        let old_page = (old_brk + crate::config::PAGE_SIZE - 1) / crate::config::PAGE_SIZE;
        let new_page = (new_brk + crate::config::PAGE_SIZE - 1) / crate::config::PAGE_SIZE;
        for vpn in old_page..new_page {
            let vpn = crate::mm::VirtPageNum(vpn);
            if p.memory_set.page_table.translate(vpn).is_none() {
                let frame = crate::mm::frame_alloc().expect("brk: out of memory");
                let ppn = frame.ppn;
                p.memory_set.page_table.map(vpn, ppn,
                    crate::mm::PTEFlags::R | crate::mm::PTEFlags::W | crate::mm::PTEFlags::U);
                core::mem::forget(frame);
            }
        }
    }
    p.brk = new_brk;
    new_brk as isize
}

fn sys_mmap(addr: usize, len: usize, prot: usize, flags: usize, fd: i32, offset: usize) -> isize {
    use crate::mm::*;
    let proc = crate::process::current_process();
    let mut p = proc.lock();

    let len_aligned = (len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

    // Choose address
    let map_addr = if addr != 0 && (flags & 0x10) != 0 {
        // MAP_FIXED
        addr & !(PAGE_SIZE - 1)
    } else if addr != 0 {
        addr & !(PAGE_SIZE - 1)
    } else {
        p.mmap_top -= len_aligned;
        p.mmap_top
    };

    let mut pte_flags = PTEFlags::U;
    if prot & 1 != 0 { pte_flags |= PTEFlags::R; }
    if prot & 2 != 0 { pte_flags |= PTEFlags::W; }
    if prot & 4 != 0 { pte_flags |= PTEFlags::X; }
    // Always add write for initial setup
    pte_flags |= PTEFlags::W;

    // Map pages
    let start_vpn = map_addr / PAGE_SIZE;
    let end_vpn = (map_addr + len_aligned) / PAGE_SIZE;
    for vpn in start_vpn..end_vpn {
        let vpn = VirtPageNum(vpn);
        if p.memory_set.page_table.translate(vpn).is_some() {
            p.memory_set.page_table.unmap(vpn);
        }
        let frame = crate::mm::frame_alloc().expect("mmap: out of memory");
        let ppn = frame.ppn;
        p.memory_set.page_table.map(vpn, ppn, pte_flags);
        core::mem::forget(frame);
    }

    // If file-backed mapping, copy file data
    let is_anonymous = (flags & 0x20) != 0; // MAP_ANONYMOUS
    if fd >= 0 && !is_anonymous {
        if let Some(fd_obj) = p.get_fd(fd as usize) {
            let f = fd_obj.lock();
            if let crate::fs::FileDescriptor::File { data, .. } = &*f {
                // Copy file data into mapped pages
                let file_len = core::cmp::min(data.len().saturating_sub(offset), len);
                if file_len > 0 {
                    let file_data = data[offset..offset + file_len].to_vec();
                    drop(f);
                    drop(p);
                    // Write file data to mapped pages
                    let proc = crate::process::current_process();
                    let p = proc.lock();
                    let mut copied = 0;
                    for vpn_val in start_vpn..end_vpn {
                        if copied >= file_data.len() { break; }
                        let vpn = VirtPageNum(vpn_val);
                        if let Some(pte) = p.memory_set.page_table.translate(vpn) {
                            let pa = pte.ppn().addr().0;
                            let page_offset = if vpn_val == start_vpn { map_addr & (PAGE_SIZE - 1) } else { 0 };
                            let copy_len = core::cmp::min(PAGE_SIZE - page_offset, file_data.len() - copied);
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    file_data[copied..].as_ptr(),
                                    (pa + page_offset) as *mut u8,
                                    copy_len,
                                );
                            }
                            copied += copy_len;
                        }
                    }
                    return map_addr as isize;
                }
            }
        }
    }
    drop(p);

    map_addr as isize
}

fn sys_munmap(addr: usize, len: usize) -> isize {
    // Simple: just unmap the pages
    let proc = crate::process::current_process();
    let mut p = proc.lock();
    let start_vpn = addr / crate::config::PAGE_SIZE;
    let end_vpn = (addr + len + crate::config::PAGE_SIZE - 1) / crate::config::PAGE_SIZE;
    for vpn in start_vpn..end_vpn {
        let vpn = crate::mm::VirtPageNum(vpn);
        if p.memory_set.page_table.translate(vpn).is_some() {
            p.memory_set.page_table.unmap(vpn);
        }
    }
    0
}

fn sys_mprotect(addr: usize, len: usize, prot: usize) -> isize {
    // Stub: just return success
    0
}

fn sys_getpid() -> isize {
    crate::process::current_pid() as isize
}

fn sys_getppid() -> isize {
    let proc = crate::process::current_process();
    let p = proc.lock();
    p.ppid as isize
}

fn sys_gettid() -> isize {
    crate::process::current_pid() as isize
}

fn sys_set_tid_address(tidptr: usize) -> isize {
    crate::process::current_pid() as isize
}

fn sys_uname(buf: usize) -> isize {
    // struct utsname: 5 fields, each 65 bytes
    let mut utsname = [0u8; 65 * 6];
    let fields = [
        "Linux",        // sysname
        "jiegeos",      // nodename
        "6.1.0",        // release
        "JiegeOS v0.1", // version
        "riscv64",      // machine
        "",             // domainname
    ];
    for (i, field) in fields.iter().enumerate() {
        let bytes = field.as_bytes();
        let start = i * 65;
        utsname[start..start + bytes.len()].copy_from_slice(bytes);
    }
    write_user_data(buf, &utsname);
    0
}

fn get_time_us() -> u64 {
    let time = riscv::register::time::read() as u64;
    time * 1_000_000 / crate::config::CLOCK_FREQ as u64
}

fn sys_clock_gettime(clockid: usize, tp: usize) -> isize {
    let us = get_time_us();
    let sec = us / 1_000_000;
    let nsec = (us % 1_000_000) * 1000;
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&(sec as u64).to_le_bytes());
    buf[8..16].copy_from_slice(&(nsec as u64).to_le_bytes());
    write_user_data(tp, &buf);
    0
}

fn sys_gettimeofday(tv: usize, _tz: usize) -> isize {
    if tv == 0 { return 0; }
    let us = get_time_us();
    let sec = us / 1_000_000;
    let usec = us % 1_000_000;
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&(sec as u64).to_le_bytes());
    buf[8..16].copy_from_slice(&(usec as u64).to_le_bytes());
    write_user_data(tv, &buf);
    0
}

fn sys_nanosleep(req: usize, _rem: usize) -> isize {
    // Read requested sleep time
    let data = read_user_bytes(req, 16);
    let sec = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let nsec = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let sleep_us = sec * 1_000_000 + nsec / 1000;
    let start = get_time_us();
    while get_time_us() - start < sleep_us {
        // Busy wait (TODO: implement proper sleeping)
        core::hint::spin_loop();
    }
    0
}

fn sys_sigaction(_signum: usize, _act: usize, _oldact: usize) -> isize {
    0 // Stub
}

fn sys_sigprocmask(_how: usize, _set: usize, _oldset: usize, _sigsetsize: usize) -> isize {
    0 // Stub
}

fn sys_openat(dirfd: i32, pathname_ptr: usize, flags: i32, mode: u32) -> isize {
    let pathname = read_user_cstr(pathname_ptr);

    // Resolve path relative to cwd
    let full_path = if pathname.starts_with('/') {
        pathname.clone()
    } else {
        let proc = crate::process::current_process();
        let p = proc.lock();
        let cwd = p.cwd.clone();
        drop(p);
        if cwd == "/" {
            alloc::format!("/{}", pathname)
        } else {
            alloc::format!("{}/{}", cwd, pathname)
        }
    };

    // Handle /dev/null
    if full_path == "/dev/null" {
        let proc = crate::process::current_process();
        let mut p = proc.lock();
        let fd = p.alloc_fd();
        p.fd_table[fd] = Some(alloc::sync::Arc::new(spin::Mutex::new(crate::fs::FileDescriptor::DevNull)));
        return fd as isize;
    }

    // Handle /proc paths
    if full_path.starts_with("/proc") {
        return -2;
    }

    // Check O_CREAT flag
    let o_creat = (flags & 0o100) != 0;
    let o_wronly = (flags & 0o3) == 1;
    let o_rdwr = (flags & 0o3) == 2;
    let o_trunc = (flags & 0o1000) != 0;
    let o_append = (flags & 0o2000) != 0;

    // Try to open from ramfs
    let fs = crate::fs::RAMFS.lock();
    if let Some(file) = fs.get_file(&full_path) {
        if file.is_dir {
            // Opening a directory
            let proc = crate::process::current_process();
            let mut p = proc.lock();
            let fd = p.alloc_fd();
            p.fd_table[fd] = Some(alloc::sync::Arc::new(spin::Mutex::new(
                crate::fs::FileDescriptor::File {
                    data: alloc::vec::Vec::new(),
                    offset: 0,
                    path: full_path,
                }
            )));
            return fd as isize;
        }
        let data = if o_trunc {
            alloc::vec::Vec::new()
        } else {
            file.data.to_vec()
        };
        drop(fs);

        let offset = if o_append { data.len() } else { 0 };
        let proc = crate::process::current_process();
        let mut p = proc.lock();
        let fd = p.alloc_fd();
        p.fd_table[fd] = Some(alloc::sync::Arc::new(spin::Mutex::new(
            crate::fs::FileDescriptor::File {
                data,
                offset,
                path: full_path,
            }
        )));
        return fd as isize;
    }
    drop(fs);

    // If O_CREAT, create a new empty file
    if o_creat {
        let proc = crate::process::current_process();
        let mut p = proc.lock();
        let fd = p.alloc_fd();
        p.fd_table[fd] = Some(alloc::sync::Arc::new(spin::Mutex::new(
            crate::fs::FileDescriptor::File {
                data: alloc::vec::Vec::new(),
                offset: 0,
                path: full_path,
            }
        )));
        return fd as isize;
    }

    // Not found
    -2 // ENOENT
}

fn sys_fstat(fd: usize, buf: usize) -> isize {
    let proc = crate::process::current_process();
    let p = proc.lock();
    if let Some(fd_obj) = p.get_fd(fd) {
        drop(p);
        let f = fd_obj.lock();
        let mut stat = [0u8; 128];
        // st_mode = regular file, 0644
        let mode: u32 = 0o100644;
        stat[16..20].copy_from_slice(&mode.to_le_bytes());
        // st_size
        let size: u64 = match &*f {
            crate::fs::FileDescriptor::File { data, .. } => data.len() as u64,
            _ => 0,
        };
        stat[48..56].copy_from_slice(&size.to_le_bytes());
        // st_blksize = 4096
        stat[56..64].copy_from_slice(&4096u64.to_le_bytes());
        // st_blocks = (size + 511) / 512
        let blocks = (size + 511) / 512;
        stat[64..72].copy_from_slice(&blocks.to_le_bytes());
        drop(f);
        write_user_data(buf, &stat);
        0
    } else {
        -9 // EBADF
    }
}

fn sys_fstatat(_dirfd: i32, pathname_ptr: usize, buf: usize, _flags: i32) -> isize {
    let pathname = read_user_cstr(pathname_ptr);
    let fs = crate::fs::RAMFS.lock();
    if fs.exists(&pathname) {
        drop(fs);
        // Fill in a reasonable stat structure
        let mut stat = [0u8; 128];
        // st_mode = regular file, 0644
        let mode: u32 = 0o100644;
        stat[16..20].copy_from_slice(&mode.to_le_bytes());
        // st_blksize = 4096
        stat[56..64].copy_from_slice(&4096u64.to_le_bytes());
        write_user_data(buf, &stat);
        return 0;
    }
    drop(fs);
    -2 // ENOENT
}

fn sys_statx(_dirfd: i32, _pathname_ptr: usize, _flags: i32, _mask: u32, _buf: usize) -> isize {
    -2 // ENOENT
}

fn sys_fcntl(fd: usize, cmd: usize, arg: usize) -> isize {
    match cmd {
        1 => 0,  // F_GETFD -> no close-on-exec
        2 => 0,  // F_SETFD
        3 => 0o2, // F_GETFL -> O_RDWR
        4 => 0,  // F_SETFL
        _ => 0,
    }
}

fn sys_ioctl(fd: usize, request: usize, arg: usize) -> isize {
    // Stub
    -25 // ENOTTY
}

fn sys_getcwd(buf: usize, size: usize) -> isize {
    let proc = crate::process::current_process();
    let p = proc.lock();
    let cwd_bytes = p.cwd.as_bytes().to_vec();
    drop(p);
    let len = core::cmp::min(cwd_bytes.len() + 1, size);
    write_user_data(buf, &cwd_bytes[..len-1]);
    write_user_data(buf + len - 1, &[0]);
    buf as isize
}

fn sys_dup(fd: usize) -> isize {
    let proc = crate::process::current_process();
    let mut p = proc.lock();
    if let Some(fd_obj) = p.get_fd(fd) {
        let new_fd = p.alloc_fd();
        p.fd_table[new_fd] = Some(fd_obj);
        new_fd as isize
    } else {
        -9
    }
}

fn sys_dup3(oldfd: usize, newfd: usize, _flags: i32) -> isize {
    let proc = crate::process::current_process();
    let mut p = proc.lock();
    if let Some(fd_obj) = p.get_fd(oldfd) {
        while p.fd_table.len() <= newfd {
            p.fd_table.push(None);
        }
        p.fd_table[newfd] = Some(fd_obj);
        newfd as isize
    } else {
        -9
    }
}

fn sys_pipe2(pipefd_ptr: usize, _flags: i32) -> isize {
    let proc = crate::process::current_process();
    let mut p = proc.lock();
    let read_fd = p.alloc_fd();
    p.fd_table[read_fd] = Some(alloc::sync::Arc::new(spin::Mutex::new(
        crate::fs::FileDescriptor::Pipe { buffer: alloc::vec::Vec::new(), read_pos: 0 }
    )));
    let write_fd = p.alloc_fd();
    p.fd_table[write_fd] = p.fd_table[read_fd].clone();
    drop(p);
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&(read_fd as u32).to_le_bytes());
    buf[4..8].copy_from_slice(&(write_fd as u32).to_le_bytes());
    write_user_data(pipefd_ptr, &buf);
    0
}

fn sys_lseek(fd: usize, offset: isize, whence: i32) -> isize {
    let proc = crate::process::current_process();
    let p = proc.lock();
    if let Some(fd_obj) = p.get_fd(fd) {
        drop(p);
        let mut f = fd_obj.lock();
        match &mut *f {
            crate::fs::FileDescriptor::File { data, offset: ref mut off, .. } => {
                match whence {
                    0 => *off = offset as usize, // SEEK_SET
                    1 => *off = (*off as isize + offset) as usize, // SEEK_CUR
                    2 => *off = (data.len() as isize + offset) as usize, // SEEK_END
                    _ => return -22,
                }
                *off as isize
            }
            _ => -29, // ESPIPE
        }
    } else {
        -9
    }
}

fn sys_prlimit64(_pid: usize, resource: usize, new_limit: usize, old_limit: usize) -> isize {
    // Return reasonable defaults
    if old_limit != 0 {
        let mut buf = [0u8; 16];
        match resource {
            7 => {
                // RLIMIT_NOFILE: soft=1024, hard=4096
                buf[0..8].copy_from_slice(&1024u64.to_le_bytes());
                buf[8..16].copy_from_slice(&4096u64.to_le_bytes());
            }
            _ => {
                // Unlimited
                let unlimited = u64::MAX;
                buf[0..8].copy_from_slice(&unlimited.to_le_bytes());
                buf[8..16].copy_from_slice(&unlimited.to_le_bytes());
            }
        }
        write_user_data(old_limit, &buf);
    }
    0
}

fn sys_getrandom(buf: usize, len: usize, _flags: usize) -> isize {
    // Simple PRNG based on timer
    let mut seed = riscv::register::time::read() as u64;
    let mut data = alloc::vec![0u8; len];
    for b in data.iter_mut() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (seed >> 33) as u8;
    }
    write_user_data(buf, &data);
    len as isize
}

fn sys_clone(flags: usize, stack: usize, ptid: usize, tls: usize, ctid: usize) -> isize {
    // TODO: implement fork/clone
    println!("[syscall] clone not implemented");
    -38
}

fn sys_execve(pathname: usize, _argv: usize, _envp: usize) -> isize {
    // TODO
    -38
}

fn sys_wait4(pid: isize, wstatus: usize, options: i32, rusage: usize) -> isize {
    // TODO
    -10 // ECHILD
}

fn sys_kill(pid: usize, sig: usize) -> isize {
    0 // Stub
}

// Network syscalls - stubs for now
fn sys_socket(domain: i32, socktype: i32, protocol: i32) -> isize { -38 }
fn sys_bind(sockfd: usize, addr: usize, addrlen: usize) -> isize { -38 }
fn sys_listen(sockfd: usize, backlog: i32) -> isize { -38 }
fn sys_accept(sockfd: usize, addr: usize, addrlen: usize) -> isize { -38 }
fn sys_connect(sockfd: usize, addr: usize, addrlen: usize) -> isize { -38 }
fn sys_setsockopt(sockfd: usize, level: i32, optname: i32, optval: usize, optlen: usize) -> isize { 0 }
fn sys_getsockopt(sockfd: usize, level: i32, optname: i32, optval: usize, optlen: usize) -> isize { -38 }
fn sys_getsockname(sockfd: usize, addr: usize, addrlen: usize) -> isize { -38 }
fn sys_getpeername(sockfd: usize, addr: usize, addrlen: usize) -> isize { -38 }
fn sys_sendto(sockfd: usize, buf: usize, len: usize, flags: i32, dest_addr: usize, addrlen: usize) -> isize { -38 }
fn sys_recvfrom(sockfd: usize, buf: usize, len: usize, flags: i32, src_addr: usize, addrlen: usize) -> isize { -38 }
fn sys_shutdown_sock(sockfd: usize, how: i32) -> isize { 0 }
fn sys_epoll_create1(flags: i32) -> isize { -38 }
fn sys_epoll_ctl(epfd: usize, op: i32, fd: usize, event: usize) -> isize { -38 }
fn sys_epoll_pwait(epfd: usize, events: usize, maxevents: i32, timeout: i32, sigmask: usize) -> isize { -38 }
fn sys_eventfd2(initval: u32, flags: i32) -> isize { -38 }
fn sys_ppoll(fds: usize, nfds: usize, tmo_p: usize, sigmask: usize) -> isize { -38 }
fn sys_socketpair(domain: i32, socktype: i32, protocol: i32, sv: usize) -> isize { -38 }

fn sys_times(buf: usize) -> isize {
    if buf != 0 {
        write_user_data(buf, &[0u8; 32]);
    }
    (riscv::register::time::read() / 10000) as isize
}

fn sys_getrusage(who: i32, usage: usize) -> isize {
    if usage != 0 {
        write_user_data(usage, &[0u8; 144]);
    }
    0
}

fn sys_setitimer(which: i32, new_value: usize, old_value: usize) -> isize {
    if old_value != 0 {
        write_user_data(old_value, &[0u8; 32]);
    }
    0
}

fn sys_sendfile(out_fd: usize, in_fd: usize, offset: usize, count: usize) -> isize {
    -38 // TODO
}

fn sys_pread64(fd: usize, buf_ptr: usize, count: usize, offset: usize) -> isize {
    let proc = crate::process::current_process();
    let p = proc.lock();
    if let Some(fd_obj) = p.get_fd(fd) {
        drop(p);
        let f = fd_obj.lock();
        match &*f {
            crate::fs::FileDescriptor::File { data, .. } => {
                if offset >= data.len() { return 0; }
                let available = data.len() - offset;
                let read_len = core::cmp::min(count, available);
                let result = data[offset..offset + read_len].to_vec();
                drop(f);
                write_user_data(buf_ptr, &result);
                read_len as isize
            }
            _ => -9,
        }
    } else {
        -9
    }
}

fn sys_readlinkat(dirfd: i32, pathname_ptr: usize, buf: usize, bufsiz: usize) -> isize {
    let pathname = read_user_cstr(pathname_ptr);
    if pathname == "/proc/self/exe" {
        let path = b"/usr/sbin/nginx";
        let len = core::cmp::min(path.len(), bufsiz);
        write_user_data(buf, &path[..len]);
        return len as isize;
    }
    -2 // ENOENT
}

fn sys_faccessat(dirfd: i32, pathname_ptr: usize, mode: i32) -> isize {
    let pathname = read_user_cstr(pathname_ptr);
    let fs = crate::fs::RAMFS.lock();
    if fs.exists(&pathname) {
        return 0;
    }
    -2 // ENOENT
}

fn sys_mkdirat(dirfd: i32, pathname_ptr: usize, mode: u32) -> isize {
    0 // Pretend success
}

fn sys_unlinkat(dirfd: i32, pathname_ptr: usize, flags: i32) -> isize {
    0 // Pretend success
}

fn sys_clock_getres(clockid: usize, res: usize) -> isize {
    if res != 0 {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&0u64.to_le_bytes());
        buf[8..16].copy_from_slice(&1000u64.to_le_bytes()); // 1us resolution
        write_user_data(res, &buf);
    }
    0
}

fn sys_getdents64(fd: usize, dirp: usize, count: usize) -> isize {
    0 // Empty directory
}
