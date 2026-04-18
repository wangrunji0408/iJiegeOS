# iJiege — a from-scratch RISC-V kernel in Rust

A small RISC-V (rv64gc) kernel that boots on QEMU `virt`, runs user programs,
speaks TCP through a virtio-net NIC + smoltcp stack, and serves HTTP to the
host through QEMU port forwarding.

## Status

**Works end-to-end:** the kernel boots, launches a user-space HTTP server, and
the host can `curl localhost:5555` to fetch a page through the QEMU NAT.

```
=== curl against the kernel ===
HTTP/1.0 200 OK
Content-Length: 160
Content-Type: text/html
Connection: close

<!doctype html><html><body><h1>Hello from iJiege RISC-V Rust kernel!</h1>
```

## What's implemented

- Boot via OpenSBI, S-mode entry, BSS clearing, primary-hart-only init
- Sv39 paging with identity-mapped kernel + MMIO + device regions
- Per-task page tables that include the kernel identity maps (no satp swap on trap)
- Kernel heap (buddy allocator, 8 MiB) and stack-frame physical allocator
- S-mode trap vector with user-trap save/restore, timer and external IRQs
- ELF64 loader with multi-segment page-union permission merging
- User-space argc/argv/envp/auxv construction on stack (glibc/musl layout)
- Scheduler + task lifecycle (Ready/Running/Zombie), round-robin via timer
- Linux-shaped syscall dispatcher with: read/write/writev, close, brk,
  clock_gettime, gettimeofday, uname, getpid/uid/gid, socket/bind/listen/accept,
  setsockopt, shutdown, exit, sched_yield, set_tid_address, rt_sigaction stubs
- virtio-mmio probe, virtio-net driver through `virtio-drivers`
- smoltcp interface at 10.0.2.15/24, TCP listen/accept/send/recv
- Socket file-descriptor abstraction wired into the Linux syscall layer
- Two demo user-space binaries: `hello` (prints a line) and `httpd`
  (listens on :80, responds with a fixed HTML page)

## How to run

Requires `qemu-system-riscv64`, Rust nightly with the RISC-V target.

```bash
make                 # builds both user binaries and the kernel
make run             # boots QEMU with user networking + port forward
curl http://localhost:5555/   # talks to the kernel's user-space httpd
```

## What's NOT implemented (vs. the original "run nginx" target)

Running an unmodified Linux nginx binary is substantially further along than
this kernel goes. The remaining gap, in rough order of effort:

1. **Dynamic ELF/PIE loading.** The Alpine nginx package is a PIE that asks
   for the interpreter `/lib/ld-musl-riscv64.so.1`. The kernel would need to
   load both the main binary at a chosen base and the interpreter at another
   base, then populate the correct AT_BASE/AT_PHDR/AT_ENTRY aux vector
   entries. Relocations happen in userspace, but the staging must be right.
2. **Filesystem.** A real FS (or initramfs) that holds `/usr/sbin/nginx`,
   `/lib/ld-musl-riscv64.so.1`, `/etc/nginx/*`, `/var/lib/nginx`,
   `/var/log/nginx/*`, mimetypes, and document root.
3. **Many more syscalls**: mmap/munmap/mprotect/madvise, openat/close,
   fstat/fstatat/newfstatat, lstat, getdents64, fcntl, ioctl (TIOCGWINSZ,
   FIONBIO, etc.), readv, pread/pwrite, lseek, sendfile, dup3, pipe2,
   getrandom, prlimit64, set_robust_list (proper), futex, epoll_create1,
   epoll_ctl, epoll_pwait, rt_sigprocmask/rt_sigaction (real semantics),
   rt_sigtimedwait, signalfd4, eventfd2, timerfd_*, clone/execve/wait4,
   getrusage, sysinfo, sched_setaffinity, kill, tgkill, setpgid, getpgid,
   setsid, umask, readlinkat, prctl, accept4 semantics beyond the trivial
   case, and several dozen more.
4. **Process + signal plumbing.** clone()/fork() semantics, wait4, signal
   delivery onto a task's trap frame with the alt-stack rules.
5. **epoll.** nginx is epoll-driven; a fake blocking polyfill is a dead end.
6. **FD table discipline.** dup, O_CLOEXEC, O_NONBLOCK, per-fd offset for
   disk files, shared tables across clones, etc.

Any one of those is not a weekend; the set together is a sustained multi-week
effort even for someone who already knows the Linux syscall surface. So this
project's "can QEMU serve HTTP from a Rust kernel the author wrote" has been
demonstrated, but "runs an unmodified nginx binary" is a different order of
magnitude of work that is not reached here.
