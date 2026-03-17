/// Linux 系统调用实现
/// RISC-V64 Linux 系统调用号

mod file;
mod process;
mod memory;
mod network;
mod signal;
mod time;
mod misc;
mod epoll;

use crate::arch::trap::TrapContext;

/// RISC-V64 Linux syscall numbers
#[allow(non_camel_case_types, dead_code)]
mod nr {
    pub const IO_SETUP: usize = 0;
    pub const IO_DESTROY: usize = 1;
    pub const IO_SUBMIT: usize = 2;
    pub const IO_CANCEL: usize = 3;
    pub const IO_GETEVENTS: usize = 4;
    pub const SETXATTR: usize = 5;
    pub const LSETXATTR: usize = 6;
    pub const FSETXATTR: usize = 7;
    pub const GETXATTR: usize = 8;
    pub const LGETXATTR: usize = 9;
    pub const FGETXATTR: usize = 10;
    pub const LISTXATTR: usize = 11;
    pub const LLISTXATTR: usize = 12;
    pub const FLISTXATTR: usize = 13;
    pub const REMOVEXATTR: usize = 14;
    pub const LREMOVEXATTR: usize = 15;
    pub const FREMOVEXATTR: usize = 16;
    pub const GETCWD: usize = 17;
    pub const LOOKUP_DCOOKIE: usize = 18;
    pub const EVENTFD2: usize = 19;
    pub const EPOLL_CREATE1: usize = 20;
    pub const EPOLL_CTL: usize = 21;
    pub const EPOLL_PWAIT: usize = 22;
    pub const DUP: usize = 23;
    pub const DUP3: usize = 24;
    pub const FCNTL: usize = 25;
    pub const INOTIFY_INIT1: usize = 26;
    pub const INOTIFY_ADD_WATCH: usize = 27;
    pub const INOTIFY_RM_WATCH: usize = 28;
    pub const IOCTL: usize = 29;
    pub const IOPRIO_SET: usize = 30;
    pub const IOPRIO_GET: usize = 31;
    pub const FLOCK: usize = 32;
    pub const MKNODAT: usize = 33;
    pub const MKDIRAT: usize = 34;
    pub const UNLINKAT: usize = 35;
    pub const SYMLINKAT: usize = 36;
    pub const LINKAT: usize = 37;
    pub const RENAMEAT: usize = 38;
    pub const UMOUNT2: usize = 39;
    pub const MOUNT: usize = 40;
    pub const PIVOT_ROOT: usize = 41;
    pub const NFSSERVCTL: usize = 42;
    pub const STATFS: usize = 43;
    pub const FSTATFS: usize = 44;
    pub const TRUNCATE: usize = 45;
    pub const FTRUNCATE: usize = 46;
    pub const FALLOCATE: usize = 47;
    pub const FACCESSAT: usize = 48;
    pub const CHDIR: usize = 49;
    pub const FCHDIR: usize = 50;
    pub const CHROOT: usize = 51;
    pub const FCHMOD: usize = 52;
    pub const FCHMODAT: usize = 53;
    pub const FCHOWNAT: usize = 54;
    pub const FCHOWN: usize = 55;
    pub const OPENAT: usize = 56;
    pub const CLOSE: usize = 57;
    pub const VHANGUP: usize = 58;
    pub const PIPE2: usize = 59;
    pub const QUOTACTL: usize = 60;
    pub const GETDENTS64: usize = 61;
    pub const LSEEK: usize = 62;
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const READV: usize = 65;
    pub const WRITEV: usize = 66;
    pub const PREAD64: usize = 67;
    pub const PWRITE64: usize = 68;
    pub const PREADV: usize = 69;
    pub const PWRITEV: usize = 70;
    pub const SENDFILE: usize = 71;
    pub const PSELECT6: usize = 72;
    pub const PPOLL: usize = 73;
    pub const SIGNALFD4: usize = 74;
    pub const VMSPLICE: usize = 75;
    pub const SPLICE: usize = 76;
    pub const TEE: usize = 77;
    pub const READLINKAT: usize = 78;
    pub const NEWFSTATAT: usize = 79;
    pub const FSTAT: usize = 80;
    pub const SYNC: usize = 81;
    pub const FSYNC: usize = 82;
    pub const FDATASYNC: usize = 83;
    pub const SYNC_FILE_RANGE: usize = 84;
    pub const TIMERFD_CREATE: usize = 85;
    pub const TIMERFD_SETTIME: usize = 86;
    pub const TIMERFD_GETTIME: usize = 87;
    pub const UTIMENSAT: usize = 88;
    pub const ACCT: usize = 89;
    pub const CAPGET: usize = 90;
    pub const CAPSET: usize = 91;
    pub const PERSONALITY: usize = 92;
    pub const EXIT: usize = 93;
    pub const EXIT_GROUP: usize = 94;
    pub const WAITID: usize = 95;
    pub const SET_TID_ADDRESS: usize = 96;
    pub const UNSHARE: usize = 97;
    pub const FUTEX: usize = 98;
    pub const SET_ROBUST_LIST: usize = 99;
    pub const GET_ROBUST_LIST: usize = 100;
    pub const NANOSLEEP: usize = 101;
    pub const GETITIMER: usize = 102;
    pub const SETITIMER: usize = 103;
    pub const KEXEC_LOAD: usize = 104;
    pub const INIT_MODULE: usize = 105;
    pub const DELETE_MODULE: usize = 106;
    pub const TIMER_CREATE: usize = 107;
    pub const TIMER_GETTIME: usize = 108;
    pub const TIMER_GETOVERRUN: usize = 109;
    pub const TIMER_SETTIME: usize = 110;
    pub const TIMER_DELETE: usize = 111;
    pub const CLOCK_SETTIME: usize = 112;
    pub const CLOCK_GETTIME: usize = 113;
    pub const CLOCK_GETRES: usize = 114;
    pub const CLOCK_NANOSLEEP: usize = 115;
    pub const SYSLOG: usize = 116;
    pub const PTRACE: usize = 117;
    pub const SCHED_SETPARAM: usize = 118;
    pub const SCHED_SETSCHEDULER: usize = 119;
    pub const SCHED_GETSCHEDULER: usize = 120;
    pub const SCHED_GETPARAM: usize = 121;
    pub const SCHED_SETAFFINITY: usize = 122;
    pub const SCHED_GETAFFINITY: usize = 123;
    pub const SCHED_YIELD: usize = 124;
    pub const SCHED_GET_PRIORITY_MAX: usize = 125;
    pub const SCHED_GET_PRIORITY_MIN: usize = 126;
    pub const SCHED_RR_GET_INTERVAL: usize = 127;
    pub const RESTART_SYSCALL: usize = 128;
    pub const KILL: usize = 129;
    pub const TKILL: usize = 130;
    pub const TGKILL: usize = 131;
    pub const SIGALTSTACK: usize = 132;
    pub const RT_SIGSUSPEND: usize = 133;
    pub const RT_SIGACTION: usize = 134;
    pub const RT_SIGPROCMASK: usize = 135;
    pub const RT_SIGPENDING: usize = 136;
    pub const RT_SIGTIMEDWAIT: usize = 137;
    pub const RT_SIGQUEUEINFO: usize = 138;
    pub const RT_SIGRETURN: usize = 139;
    pub const SETPRIORITY: usize = 140;
    pub const GETPRIORITY: usize = 141;
    pub const REBOOT: usize = 142;
    pub const SETREGID: usize = 143;
    pub const SETGID: usize = 144;
    pub const SETREUID: usize = 145;
    pub const SETUID: usize = 146;
    pub const SETRESUID: usize = 147;
    pub const GETRESUID: usize = 148;
    pub const SETRESGID: usize = 149;
    pub const GETRESGID: usize = 150;
    pub const SETFSUID: usize = 151;
    pub const SETFSGID: usize = 152;
    pub const TIMES: usize = 153;
    pub const SETPGID: usize = 154;
    pub const GETPGID: usize = 155;
    pub const GETSID: usize = 156;
    pub const SETSID: usize = 157;
    pub const GETGROUPS: usize = 158;
    pub const SETGROUPS: usize = 159;
    pub const UNAME: usize = 160;
    pub const SETHOSTNAME: usize = 161;
    pub const SETDOMAINNAME: usize = 162;
    pub const GETRLIMIT: usize = 163;
    pub const SETRLIMIT: usize = 164;
    pub const GETRUSAGE: usize = 165;
    pub const UMASK: usize = 166;
    pub const PRCTL: usize = 167;
    pub const GETCPU: usize = 168;
    pub const GETTIMEOFDAY: usize = 169;
    pub const SETTIMEOFDAY: usize = 170;
    pub const ADJTIMEX: usize = 171;
    pub const GETPID: usize = 172;
    pub const GETPPID: usize = 173;
    pub const GETUID: usize = 174;
    pub const GETEUID: usize = 175;
    pub const GETGID: usize = 176;
    pub const GETEGID: usize = 177;
    pub const GETTID: usize = 178;
    pub const SYSINFO: usize = 179;
    pub const MQ_OPEN: usize = 180;
    pub const MQ_UNLINK: usize = 181;
    pub const MQ_TIMEDSEND: usize = 182;
    pub const MQ_TIMEDRECEIVE: usize = 183;
    pub const MQ_NOTIFY: usize = 184;
    pub const MQ_GETSETATTR: usize = 185;
    pub const MSGGET: usize = 186;
    pub const MSGCTL: usize = 187;
    pub const MSGRCV: usize = 188;
    pub const MSGSND: usize = 189;
    pub const SEMGET: usize = 190;
    pub const SEMCTL: usize = 191;
    pub const SEMTIMEDOP: usize = 192;
    pub const SEMOP: usize = 193;
    pub const SHMGET: usize = 194;
    pub const SHMCTL: usize = 195;
    pub const SHMAT: usize = 196;
    pub const SHMDT: usize = 197;
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
    pub const SENDMSG: usize = 211;
    pub const RECVMSG: usize = 212;
    pub const READAHEAD: usize = 213;
    pub const BRK: usize = 214;
    pub const MUNMAP: usize = 215;
    pub const MREMAP: usize = 216;
    pub const ADD_KEY: usize = 217;
    pub const REQUEST_KEY: usize = 218;
    pub const KEYCTL: usize = 219;
    pub const CLONE: usize = 220;
    pub const EXECVE: usize = 221;
    pub const MMAP: usize = 222;
    pub const FADVISE64: usize = 223;
    pub const SWAPON: usize = 224;
    pub const SWAPOFF: usize = 225;
    pub const MPROTECT: usize = 226;
    pub const MSYNC: usize = 227;
    pub const MLOCK: usize = 228;
    pub const MUNLOCK: usize = 229;
    pub const MLOCKALL: usize = 230;
    pub const MUNLOCKALL: usize = 231;
    pub const MINCORE: usize = 232;
    pub const MADVISE: usize = 233;
    pub const REMAP_FILE_PAGES: usize = 234;
    pub const MBIND: usize = 235;
    pub const GET_MEMPOLICY: usize = 236;
    pub const SET_MEMPOLICY: usize = 237;
    pub const MIGRATE_PAGES: usize = 238;
    pub const MOVE_PAGES: usize = 239;
    pub const RT_TGSIGQUEUEINFO: usize = 240;
    pub const PERF_EVENT_OPEN: usize = 241;
    pub const ACCEPT4: usize = 242;
    pub const RECVMMSG: usize = 243;
    pub const WAIT4: usize = 260;
    pub const PRLIMIT64: usize = 261;
    pub const FANOTIFY_INIT: usize = 262;
    pub const FANOTIFY_MARK: usize = 263;
    pub const NAME_TO_HANDLE_AT: usize = 264;
    pub const OPEN_BY_HANDLE_AT: usize = 265;
    pub const CLOCK_ADJTIME: usize = 266;
    pub const SYNCFS: usize = 267;
    pub const SETNS: usize = 268;
    pub const SENDMMSG: usize = 269;
    pub const PROCESS_VM_READV: usize = 270;
    pub const PROCESS_VM_WRITEV: usize = 271;
    pub const KCMP: usize = 272;
    pub const FINIT_MODULE: usize = 273;
    pub const SCHED_SETATTR: usize = 274;
    pub const SCHED_GETATTR: usize = 275;
    pub const RENAMEAT2: usize = 276;
    pub const SECCOMP: usize = 277;
    pub const GETRANDOM: usize = 278;
    pub const MEMFD_CREATE: usize = 279;
    pub const BPF: usize = 280;
    pub const EXECVEAT: usize = 281;
    pub const USERFAULTFD: usize = 282;
    pub const MEMBARRIER: usize = 283;
    pub const MLOCK2: usize = 284;
    pub const COPY_FILE_RANGE: usize = 285;
    pub const PREADV2: usize = 286;
    pub const PWRITEV2: usize = 287;
    pub const PKEY_MPROTECT: usize = 288;
    pub const PKEY_ALLOC: usize = 289;
    pub const PKEY_FREE: usize = 290;
    pub const STATX: usize = 291;
    pub const IO_PGETEVENTS: usize = 292;
    pub const RSEQ: usize = 293;
    pub const PIDFD_SEND_SIGNAL: usize = 424;
    pub const IO_URING_SETUP: usize = 425;
    pub const IO_URING_ENTER: usize = 426;
    pub const IO_URING_REGISTER: usize = 427;
    pub const OPEN_TREE: usize = 428;
    pub const MOVE_MOUNT: usize = 429;
    pub const FSOPEN: usize = 430;
    pub const FSCONFIG: usize = 431;
    pub const FSMOUNT: usize = 432;
    pub const FSPICK: usize = 433;
    pub const PIDFD_OPEN: usize = 434;
    pub const CLONE3: usize = 435;
    pub const CLOSE_RANGE: usize = 436;
    pub const OPENAT2: usize = 437;
    pub const PIDFD_GETFD: usize = 438;
    pub const FACCESSAT2: usize = 439;
    pub const PROCESS_MADVISE: usize = 440;
    pub const EPOLL_PWAIT2: usize = 441;
    pub const MOUNT_SETATTR: usize = 442;
    pub const QUOTACTL_FD: usize = 443;
    pub const LANDLOCK_CREATE_RULESET: usize = 444;
    pub const LANDLOCK_ADD_RULE: usize = 445;
    pub const LANDLOCK_RESTRICT_SELF: usize = 446;
    pub const MEMFD_SECRET: usize = 447;
    pub const PROCESS_MRELEASE: usize = 448;
}

/// 错误码
pub mod errno {
    pub const EPERM: i64 = -1;
    pub const ENOENT: i64 = -2;
    pub const ESRCH: i64 = -3;
    pub const EINTR: i64 = -4;
    pub const EIO: i64 = -5;
    pub const ENXIO: i64 = -6;
    pub const E2BIG: i64 = -7;
    pub const ENOEXEC: i64 = -8;
    pub const EBADF: i64 = -9;
    pub const ECHILD: i64 = -10;
    pub const EAGAIN: i64 = -11;
    pub const ENOMEM: i64 = -12;
    pub const EACCES: i64 = -13;
    pub const EFAULT: i64 = -14;
    pub const EBUSY: i64 = -16;
    pub const EEXIST: i64 = -17;
    pub const ENODEV: i64 = -19;
    pub const ENOTDIR: i64 = -20;
    pub const EISDIR: i64 = -21;
    pub const EINVAL: i64 = -22;
    pub const ENFILE: i64 = -23;
    pub const EMFILE: i64 = -24;
    pub const ENOTTY: i64 = -25;
    pub const ENOSPC: i64 = -28;
    pub const EPIPE: i64 = -32;
    pub const ERANGE: i64 = -34;
    pub const ENAMETOOLONG: i64 = -36;
    pub const ENOSYS: i64 = -38;
    pub const ENOTEMPTY: i64 = -39;
    pub const ELOOP: i64 = -40;
    pub const EWOULDBLOCK: i64 = EAGAIN;
    pub const ENOTSOCK: i64 = -88;
    pub const EDESTADDRREQ: i64 = -89;
    pub const EMSGSIZE: i64 = -90;
    pub const ENOPROTOOPT: i64 = -92;
    pub const EAFNOSUPPORT: i64 = -97;
    pub const EADDRINUSE: i64 = -98;
    pub const ECONNREFUSED: i64 = -111;
    pub const ETIMEDOUT: i64 = -110;
    pub const ECONNRESET: i64 = -104;
    pub const ENOTCONN: i64 = -107;
    pub const ESHUTDOWN: i64 = -108;
    pub const EISCONN: i64 = -106;
    pub const EOPNOTSUPP: i64 = -95;
}

use errno::*;

/// 系统调用分发
pub fn syscall(id: usize, args: [usize; 6], ctx: &mut TrapContext) -> i64 {
    let result = match id {
        // 文件操作
        nr::READ => file::sys_read(args[0], args[1] as *mut u8, args[2]),
        nr::WRITE => file::sys_write(args[0], args[1] as *const u8, args[2]),
        nr::OPENAT => file::sys_openat(args[0] as i32, args[1] as *const u8, args[2] as i32, args[3] as u32),
        nr::CLOSE => file::sys_close(args[0]),
        nr::FSTAT => file::sys_fstat(args[0], args[1] as *mut crate::fs::FileStat),
        nr::NEWFSTATAT => file::sys_newfstatat(args[0] as i32, args[1] as *const u8, args[2] as *mut crate::fs::FileStat, args[3] as i32),
        nr::LSEEK => file::sys_lseek(args[0], args[1] as i64, args[2] as i32),
        nr::READV => file::sys_readv(args[0], args[1], args[2]),
        nr::WRITEV => file::sys_writev(args[0], args[1], args[2]),
        nr::PREAD64 => file::sys_pread64(args[0], args[1] as *mut u8, args[2], args[3] as i64),
        nr::PWRITE64 => file::sys_pwrite64(args[0], args[1] as *const u8, args[2], args[3] as i64),
        nr::DUP => file::sys_dup(args[0]),
        nr::DUP3 => file::sys_dup3(args[0], args[1], args[2] as i32),
        nr::PIPE2 => file::sys_pipe2(args[0] as *mut i32, args[1] as i32),
        nr::FCNTL => file::sys_fcntl(args[0], args[1] as i32, args[2]),
        nr::IOCTL => file::sys_ioctl(args[0], args[1] as u64, args[2]),
        nr::GETDENTS64 => file::sys_getdents64(args[0], args[1] as *mut u8, args[2]),
        nr::READLINKAT => file::sys_readlinkat(args[0] as i32, args[1] as *const u8, args[2] as *mut u8, args[3]),
        nr::FACCESSAT | nr::FACCESSAT2 => file::sys_faccessat(args[0] as i32, args[1] as *const u8, args[2] as i32, args[3] as i32),
        nr::MKDIRAT => file::sys_mkdirat(args[0] as i32, args[1] as *const u8, args[2] as u32),
        nr::UNLINKAT => file::sys_unlinkat(args[0] as i32, args[1] as *const u8, args[2] as i32),
        nr::RENAMEAT => file::sys_renameat(args[0] as i32, args[1] as *const u8, args[2] as i32, args[3] as *const u8),
        nr::FTRUNCATE => file::sys_ftruncate(args[0], args[1] as i64),
        nr::FLOCK => file::sys_flock(args[0], args[1] as i32),
        nr::SENDFILE => file::sys_sendfile(args[0], args[1], args[2] as *mut i64, args[3]),
        nr::GETCWD => file::sys_getcwd(args[0] as *mut u8, args[1]),
        nr::CHDIR => file::sys_chdir(args[0] as *const u8),
        nr::FCHDIR => file::sys_fchdir(args[0]),
        nr::FCHMOD => file::sys_fchmod(args[0], args[1] as u32),
        nr::FCHMODAT => file::sys_fchmodat(args[0] as i32, args[1] as *const u8, args[2] as u32),
        nr::FCHOWNAT => 0,  // stub
        nr::FCHOWN => 0,    // stub
        nr::UTIMENSAT => 0, // stub
        nr::STATFS | nr::FSTATFS => file::sys_statfs(args[0] as *const u8, args[1] as *mut u8),
        nr::SYMLINKAT => file::sys_symlinkat(args[0] as *const u8, args[1] as i32, args[2] as *const u8),
        nr::LINKAT => file::sys_linkat(args[0] as i32, args[1] as *const u8, args[2] as i32, args[3] as *const u8, args[4] as i32),
        nr::FSYNC | nr::FDATASYNC | nr::SYNC => 0,  // stub
        nr::FALLOCATE => 0,  // stub

        // 进程管理
        nr::GETPID => process::sys_getpid(),
        nr::GETPPID => process::sys_getppid(),
        nr::GETTID => process::sys_gettid(),
        nr::GETUID | nr::GETEUID => process::sys_getuid(),
        nr::GETGID | nr::GETEGID => process::sys_getgid(),
        nr::SETUID | nr::SETREUID | nr::SETRESUID => 0,
        nr::SETGID | nr::SETREGID | nr::SETRESGID => 0,
        nr::GETRESUID => process::sys_getresuid(args[0], args[1], args[2]),
        nr::GETRESGID => process::sys_getresgid(args[0], args[1], args[2]),
        nr::SETGROUPS => 0,
        nr::GETGROUPS => process::sys_getgroups(args[0] as i32, args[1]),
        nr::SETPGID => process::sys_setpgid(args[0] as i32, args[1] as i32),
        nr::GETPGID => process::sys_getpgid(args[0] as i32),
        nr::GETSID => process::sys_getsid(args[0] as i32),
        nr::SETSID => process::sys_setsid(),
        // RISC-V clone 参数顺序: flags, newsp, parent_tidptr, tls, child_tidptr
        nr::CLONE => process::sys_clone(args[0], args[1], args[2], args[4], args[3], ctx),
        nr::EXECVE => process::sys_execve(args[0] as *const u8, args[1] as *const usize, args[2] as *const usize, ctx),
        nr::WAIT4 | nr::WAITID => process::sys_wait4(args[0] as i32, args[1], args[2] as i32),
        nr::EXIT => process::sys_exit(args[0] as i32),
        nr::EXIT_GROUP => process::sys_exit_group(args[0] as i32),
        nr::SET_TID_ADDRESS => process::sys_set_tid_address(args[0]),
        nr::PRCTL => process::sys_prctl(args[0] as i32, args[1], args[2], args[3], args[4]),
        nr::GETRLIMIT => process::sys_getrlimit(args[0] as u32, args[1] as *mut u64),
        nr::SETRLIMIT => process::sys_setrlimit(args[0] as u32, args[1] as *const u64),
        nr::PRLIMIT64 => process::sys_prlimit64(args[0] as i32, args[1] as u32, args[2] as *const u64, args[3] as *mut u64),
        nr::GETRUSAGE => process::sys_getrusage(args[0] as i32, args[1]),
        nr::UMASK => process::sys_umask(args[0] as u32),
        nr::PERSONALITY => process::sys_personality(args[0] as u32),
        nr::CAPGET | nr::CAPSET => 0,  // stub

        // 内存管理
        nr::BRK => memory::sys_brk(args[0]),
        nr::MMAP => memory::sys_mmap(args[0], args[1], args[2] as i32, args[3] as i32, args[4] as i32, args[5] as i64),
        nr::MUNMAP => memory::sys_munmap(args[0], args[1]),
        nr::MPROTECT => memory::sys_mprotect(args[0], args[1], args[2] as i32),
        nr::MADVISE => 0,  // stub
        nr::MSYNC => 0,    // stub
        nr::MLOCK | nr::MUNLOCK | nr::MLOCKALL | nr::MUNLOCKALL | nr::MLOCK2 => 0,
        nr::MINCORE => 0,  // stub

        // 网络
        nr::SOCKET => network::sys_socket(args[0] as i32, args[1] as i32, args[2] as i32),
        nr::BIND => network::sys_bind(args[0], args[1] as *const u8, args[2] as u32),
        nr::LISTEN => network::sys_listen(args[0], args[1] as i32),
        nr::ACCEPT => network::sys_accept(args[0], args[1] as *mut u8, args[2] as *mut u32),
        nr::ACCEPT4 => network::sys_accept4(args[0], args[1] as *mut u8, args[2] as *mut u32, args[3] as i32),
        nr::CONNECT => network::sys_connect(args[0], args[1] as *const u8, args[2] as u32),
        nr::GETSOCKNAME => network::sys_getsockname(args[0], args[1] as *mut u8, args[2] as *mut u32),
        nr::GETPEERNAME => network::sys_getpeername(args[0], args[1] as *mut u8, args[2] as *mut u32),
        nr::SENDTO => network::sys_sendto(args[0], args[1] as *const u8, args[2], args[3] as i32, args[4] as *const u8, args[5] as u32),
        nr::RECVFROM => network::sys_recvfrom(args[0], args[1] as *mut u8, args[2], args[3] as i32, args[4] as *mut u8, args[5] as *mut u32),
        nr::SETSOCKOPT => network::sys_setsockopt(args[0], args[1] as i32, args[2] as i32, args[3] as *const u8, args[4] as u32),
        nr::GETSOCKOPT => network::sys_getsockopt(args[0], args[1] as i32, args[2] as i32, args[3] as *mut u8, args[4] as *mut u32),
        nr::SHUTDOWN => network::sys_shutdown(args[0], args[1] as i32),
        nr::SENDMSG => network::sys_sendmsg(args[0], args[1], args[2] as i32),
        nr::RECVMSG => network::sys_recvmsg(args[0], args[1], args[2] as i32),
        nr::SOCKETPAIR => network::sys_socketpair(args[0] as i32, args[1] as i32, args[2] as i32, args[3] as *mut i32),

        // 信号
        nr::RT_SIGACTION => signal::sys_rt_sigaction(args[0] as i32, args[1] as *const u8, args[2] as *mut u8, args[3]),
        nr::RT_SIGPROCMASK => signal::sys_rt_sigprocmask(args[0] as i32, args[1] as *const u64, args[2] as *mut u64, args[3]),
        nr::RT_SIGRETURN => signal::sys_rt_sigreturn(ctx),
        nr::KILL => signal::sys_kill(args[0] as i32, args[1] as i32),
        nr::TKILL | nr::TGKILL => signal::sys_tkill(args[0] as i32, args[1] as i32, args[2] as i32),
        nr::SIGALTSTACK => 0,  // stub
        nr::RT_SIGSUSPEND => signal::sys_rt_sigsuspend(args[0] as *const u64, args[1]),

        // 时间
        nr::CLOCK_GETTIME => time::sys_clock_gettime(args[0] as i32, args[1] as *mut crate::timer::TimeSpec),
        nr::CLOCK_GETRES => time::sys_clock_getres(args[0] as i32, args[1] as *mut crate::timer::TimeSpec),
        nr::CLOCK_NANOSLEEP => time::sys_clock_nanosleep(args[0] as i32, args[1] as i32, args[2] as *const crate::timer::TimeSpec, args[3] as *mut crate::timer::TimeSpec),
        nr::NANOSLEEP => time::sys_nanosleep(args[0] as *const crate::timer::TimeSpec, args[1] as *mut crate::timer::TimeSpec),
        nr::GETTIMEOFDAY => time::sys_gettimeofday(args[0] as *mut crate::timer::TimeVal, args[1]),
        nr::TIMES => time::sys_times(args[0]),

        // Epoll
        nr::EPOLL_CREATE1 => epoll::sys_epoll_create1(args[0] as i32),
        nr::EPOLL_CTL => epoll::sys_epoll_ctl(args[0], args[1] as i32, args[2], args[3] as *const u8),
        nr::EPOLL_PWAIT | nr::EPOLL_PWAIT2 => epoll::sys_epoll_pwait(args[0], args[1] as *mut u8, args[2] as i32, args[3] as i32, args[4] as *const u64),

        // Select/Poll
        nr::PSELECT6 => epoll::sys_pselect6(args[0] as i32, args[1], args[2], args[3], args[4], args[5]),
        nr::PPOLL => epoll::sys_ppoll(args[0] as *mut u8, args[1], args[2] as *const crate::timer::TimeSpec, args[3] as *const u64),

        // 杂项
        nr::UNAME => misc::sys_uname(args[0] as *mut u8),
        nr::SYSINFO => misc::sys_sysinfo(args[0] as *mut u8),
        nr::GETRANDOM => misc::sys_getrandom(args[0] as *mut u8, args[1], args[2] as u32),
        nr::SCHED_YIELD => { crate::task::suspend_current_and_run_next(); 0 },
        nr::SCHED_GETSCHEDULER => 0,
        nr::SCHED_SETSCHEDULER => 0,
        nr::SCHED_SETPARAM => 0,
        nr::SCHED_GETPARAM => 0,
        nr::SCHED_SETAFFINITY => 0,
        nr::SCHED_GETAFFINITY => misc::sys_sched_getaffinity(args[0], args[1], args[2] as *mut u8),
        nr::FUTEX => misc::sys_futex(args[0], args[1] as i32, args[2] as u32, args[3], args[4], args[5] as u32),
        nr::SET_ROBUST_LIST => 0,
        nr::GET_ROBUST_LIST => 0,
        nr::REBOOT => { crate::arch::sbi::shutdown() },
        nr::MEMBARRIER => 0,
        nr::RSEQ => ENOSYS,
        nr::EVENTFD2 => misc::sys_eventfd2(args[0] as u32, args[1] as i32),
        nr::TIMERFD_CREATE => misc::sys_timerfd_create(args[0] as i32, args[1] as i32),
        nr::TIMERFD_SETTIME => 0,
        nr::TIMERFD_GETTIME => 0,
        nr::INOTIFY_INIT1 => misc::sys_inotify_init1(args[0] as i32),
        nr::INOTIFY_ADD_WATCH => ENOSYS,
        nr::INOTIFY_RM_WATCH => ENOSYS,
        nr::SECCOMP => ENOSYS,
        nr::CLOSE_RANGE => file::sys_close_range(args[0] as u32, args[1] as u32, args[2] as i32),

        // Linux AIO (io_setup/io_submit/io_getevents/io_destroy/io_cancel)
        // nginx uses these; return EAGAIN/success to prevent fatal error
        nr::IO_SETUP => {
            // io_setup(nr_events, ctx_idp) - 模拟成功，写入一个非零的 ctx
            // ctx_idp 是指向 io_context_t 的指针
            let ctx_idp = args[1];
            if ctx_idp != 0 {
                let tok = crate::task::current_user_token();
                *crate::mm::translated_refmut(tok, ctx_idp as *mut usize) = 0x1234abcd;  // fake ctx
            }
            0
        }
        nr::IO_DESTROY => 0,  // io_destroy(ctx) - 成功
        nr::IO_SUBMIT => 0,   // io_submit(ctx, nr, iocbpp) - 返回 0 (没有提交)
        nr::IO_CANCEL => errno::EINVAL,  // io_cancel - 不支持
        nr::IO_GETEVENTS => 0,  // io_getevents - 返回 0 (没有事件)

        _ => {
            log::debug!("Unknown syscall: {} args={:?}", id, &args[..3]);
            ENOSYS
        }
    };

    result
}
