#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

const SYS_WRITE: usize = 64;
const SYS_EXIT: usize = 93;
const SYS_CLOSE: usize = 57;
const SYS_READ: usize = 63;
const SYS_SOCKET: usize = 198;
const SYS_BIND: usize = 200;
const SYS_LISTEN: usize = 201;
const SYS_ACCEPT: usize = 202;

const AF_INET: usize = 2;
const SOCK_STREAM: usize = 1;

unsafe fn sys3(n: usize, a: usize, b: usize, c: usize) -> isize {
    let r: isize;
    asm!("ecall",
        in("a7") n,
        inlateout("a0") a => r,
        in("a1") b,
        in("a2") c);
    r
}

fn write(fd: i32, buf: &[u8]) -> isize {
    unsafe { sys3(SYS_WRITE, fd as usize, buf.as_ptr() as usize, buf.len()) }
}

fn exit(code: i32) -> ! {
    unsafe { sys3(SYS_EXIT, code as usize, 0, 0); }
    loop {}
}

fn puts(s: &str) { write(1, s.as_bytes()); }
fn put_num(mut n: u64) {
    let mut buf = [0u8; 20]; let mut i = 0;
    if n == 0 { write(1, b"0"); return; }
    while n > 0 { buf[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
    for j in (0..i).rev() { write(1, &[buf[j]]); }
}

#[repr(C)]
struct SockAddrIn { family: u16, port: u16, addr: u32, zero: [u8; 8] }

#[no_mangle]
pub extern "C" fn _start() -> ! {
    puts("httpd: starting\n");
    let sock = unsafe { sys3(SYS_SOCKET, AF_INET, SOCK_STREAM, 0) };
    if sock < 0 { puts("socket failed\n"); exit(1); }

    let sa = SockAddrIn { family: AF_INET as u16, port: (80u16).to_be(), addr: 0, zero: [0; 8] };
    let r = unsafe { sys3(SYS_BIND, sock as usize, &sa as *const _ as usize, core::mem::size_of::<SockAddrIn>()) };
    if r < 0 { puts("bind failed\n"); exit(1); }

    let r = unsafe { sys3(SYS_LISTEN, sock as usize, 16, 0) };
    if r < 0 { puts("listen failed\n"); exit(1); }
    puts("httpd: listening on port 80\n");

    let body: &[u8] = b"<!doctype html><html><body><h1>Hello from iJiege RISC-V Rust kernel!</h1><p>This page is served by a from-scratch Rust kernel running in QEMU.</p></body></html>";
    let mut hdr_buf = [0u8; 128];
    let mut hdr_len = 0usize;
    // Manually build header: HTTP/1.0 200 OK\r\nContent-Length: N\r\nContent-Type: text/html\r\n\r\n
    for &b in b"HTTP/1.0 200 OK\r\nContent-Length: " { hdr_buf[hdr_len] = b; hdr_len += 1; }
    let mut n = body.len();
    let mut digits = [0u8; 8]; let mut di = 0;
    if n == 0 { digits[0] = b'0'; di = 1; }
    while n > 0 { digits[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
    for j in (0..di).rev() { hdr_buf[hdr_len] = digits[j]; hdr_len += 1; }
    for &b in b"\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n" { hdr_buf[hdr_len] = b; hdr_len += 1; }

    let mut req = [0u8; 2048];
    let mut conn_count = 0u64;
    loop {
        let cfd = unsafe { sys3(SYS_ACCEPT, sock as usize, 0, 0) };
        if cfd < 0 { puts("accept failed\n"); continue; }
        conn_count += 1;
        puts("httpd: conn #");
        put_num(conn_count);
        puts("\n");

        // Read whatever the client sent (partial request is fine)
        let _ = unsafe { sys3(SYS_READ, cfd as usize, req.as_mut_ptr() as usize, req.len()) };

        write(cfd as i32, &hdr_buf[..hdr_len]);
        write(cfd as i32, body);
        unsafe { sys3(SYS_CLOSE, cfd as usize, 0, 0); }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! { exit(1); }
