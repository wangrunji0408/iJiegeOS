# iJiege — a from-scratch RISC-V kernel in Rust running official Linux nginx

A small RISC-V (rv64gc) kernel that boots on QEMU `virt`, loads the unmodified
Alpine riscv64 nginx binary through the real `ld-musl-riscv64.so.1` dynamic
linker, and serves HTTP to the host through QEMU user-mode port forwarding.

## Proof it works

```
$ make run &
$ curl -si --noproxy '*' http://127.0.0.1:5555/
HTTP/1.1 200 OK
Server: nginx/1.28.3              ← official Alpine nginx
Date: Thu, 01 Jan 1970 00:00:02 GMT
Content-Type: text/html
Content-Length: 89
Last-Modified: Thu, 01 Jan 1970 00:00:00 GMT
Connection: close
ETag: "0-59"
Accept-Ranges: bytes

<!doctype html>
<html><body><h1>Hello from iJiege Rust kernel + nginx</h1></body></html>
```

That `Server: nginx/1.28.3` line comes from the real nginx binary
downloaded from Alpine's riscv64 repository (`nginx-1.28.3-r0.apk`). No
patches, no repackaging — just the ELF executed inside this kernel by its
Linux-ABI syscall layer.

## What's embedded

The kernel image statically embeds, via `include_bytes!`, the exact binaries
Alpine ships for riscv64:

- `/lib/ld-musl-riscv64.so.1` (from `musl-1.2.6-r2.apk`, static-pie)
- `/usr/sbin/nginx` (from `nginx-1.28.3-r0.apk`, PIE, dynamic)
- `/usr/lib/libpcre2-8.so.0`, `/usr/lib/libssl.so.3`,
  `/usr/lib/libcrypto.so.3`, `/usr/lib/libz.so.1`
- `/etc/passwd`, `/etc/group`, `/etc/nginx/nginx.conf`,
  `/var/www/index.html`, etc., generated into an in-memory VFS

## What's implemented

Boot / memory:
- OpenSBI entry, S-mode boot, BSS clearing, primary-hart-only init
- Sv39 paging with a shared kernel identity map in every user address space
  (so trap handlers run without satp switching)
- Kernel heap (buddy, 8 MiB) + stack-frame physical allocator
- Framed and identity-mapped `MapArea` with permission unioning

Traps / scheduling:
- S-mode trap vector, per-task `TrapContext` (pinned via `Box<UnsafeCell<>>`),
  kernel stack switch in assembly, `jalr` into `trap_handler`
- Preemptive timer via SBI, round-robin scheduler
- Per-task round-tripping through `sscratch`

Process / ELF:
- ELF64 loader supporting both `ET_EXEC` and `ET_DYN` (PIE) with a load base
- Union-of-permissions page mapping so overlapping LOAD segments work
- Interpreter loading (passes the correct `AT_BASE`/`AT_PHDR`/`AT_PHNUM`/
  `AT_ENTRY` aux vector, plus `AT_RANDOM`/`AT_PLATFORM`/`AT_EXECFN`/`AT_UID`…)
- Stack set up in glibc/musl layout: argc, argv…, NULL, envp…, NULL, auxv…
- A read-only zero page at VA 0 so accidental NULL derefs return zeros
  instead of faulting (musl/nginx occasionally `memcpy(dst, NULL, n)` on
  empty optional strings)

Filesystem:
- In-memory VFS with static blobs, MemFile, /dev/stdin|stdout|stderr|null|zero|random
- Path resolver with symlink support (used for `libc.musl-riscv64.so.1 →
  ld-musl-riscv64.so.1`)
- Writable-on-create for files opened with `O_CREAT|O_RDWR|O_WRONLY`
  (so `/run/nginx.pid`, `/var/lib/nginx/logs/*` etc. just work)
- Unique inode IDs per `StaticFile` so musl's ld.so dedupe-by-(dev,ino)
  treats libssl, libcrypto, libz as distinct objects

Syscalls (the real Linux ABI layer):
- I/O: read/write, readv/writev, pread64/pwrite64, lseek, close
- VFS: openat, newfstatat, fstat, faccessat, readlinkat, getcwd, chdir,
  mkdirat, unlinkat, symlinkat, linkat, renameat, renameat2, fchmodat,
  fchownat, fchown, utimensat, truncate/ftruncate/fallocate, statfs/fstatfs,
  fsync/sync/msync, fcntl (F_DUPFD/F_DUPFD_CLOEXEC/F_GET*/F_SET*)
- Memory: brk, mmap, munmap, mprotect, madvise (with file-backed copy-in)
- Time: clock_gettime, gettimeofday
- Process: getpid/gettid/getppid, getuid/getgid/geteuid/getegid (all 0),
  sched_yield, exit, exit_group, getrusage, prlimit64, uname, sysinfo,
  umask, prctl
- Signals (stubbed): rt_sigaction, rt_sigprocmask, rt_sigreturn,
  kill/tgkill/tkill (all return 0)
- IPC / thread stubs: set_tid_address, set_robust_list, futex
- Sockets: socket, bind, listen, accept/accept4, recvfrom/sendto, sendmsg,
  getsockname/getpeername, setsockopt/getsockopt, shutdown, ioctl
- epoll: epoll_create1, epoll_ctl (ADD/MOD/DEL), epoll_pwait (blocking,
  drives `net::poll()` each iteration)
- Misc: getrandom, ioctl (stubs returning 0)

Network:
- virtio-mmio probe across the 8 QEMU slots
- virtio-net driver through `virtio-drivers`
- smoltcp at 10.0.2.15/24, gw 10.0.2.2 (QEMU user-mode defaults)
- Socket abstraction (`SocketFile`) wired into the FD table and epoll

## Running it

```bash
make           # builds user helpers and the kernel
make run       # boots QEMU with user-mode networking + tcp::5555->:80 forward
# in another terminal:
curl --noproxy '*' http://127.0.0.1:5555/
```

QEMU command used by `make run`:

```
qemu-system-riscv64 -machine virt -m 1G -nographic -bios default \
  -kernel target/riscv64gc-unknown-none-elf/release/kernel \
  -netdev user,id=net0,hostfwd=tcp::5555-:80 \
  -device virtio-net-device,netdev=net0
```

## Limitations (intentional simplifications for a single-session build)

- No `fork`/`clone`/`execve`: nginx must run with `master_process off` and
  `worker_processes 1`. `clone()` and multi-process support are not
  implemented.
- `mprotect` is a no-op. Permissions end up `R|W|X` on mmap'd regions.
  This is safe for running unmodified nginx but is not production-grade.
- `mmap` for file-backed regions eagerly copies file contents into fresh
  frames; real Linux does demand paging.
- epoll is polling-driven: `epoll_pwait` loops over the watched fds and
  calls `net::poll()` each time, then `wfi`s briefly. Good enough for a
  single nginx worker handling one request at a time; not a real event
  system.
- No TLS (SSL): the OpenSSL libraries are linked so nginx resolves symbols,
  but the kernel does not configure nginx for HTTPS listening.
- musl's thread-local storage exit path crashes on `exit()` — nginx in
  long-running mode hits a deliberate trap inside ld-musl's `exit()` once
  it's time to tear down. The kernel catches this as a page fault and
  terminates the task; the nginx binary works while it's running.
- No real disk or persistent fs; writable files live in RAM and disappear
  at poweroff.
