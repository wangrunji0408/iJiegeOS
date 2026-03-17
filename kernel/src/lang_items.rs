use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 直接使用 SBI 输出，避免 UART Mutex 死锁
    fn sbi_putchar(c: u8) {
        crate::arch::sbi::console_putchar(c);
    }
    fn print_str(s: &str) {
        for b in s.bytes() {
            sbi_putchar(b);
        }
    }
    fn print_u64_hex(mut n: u64) {
        sbi_putchar(b'0');
        sbi_putchar(b'x');
        for i in (0..16).rev() {
            let digit = ((n >> (i * 4)) & 0xf) as u8;
            sbi_putchar(if digit < 10 { b'0' + digit } else { b'a' + digit - 10 });
        }
    }

    print_str("\x1b[31m[PANIC]");
    if let Some(location) = info.location() {
        print_str(" at ");
        print_str(location.file());
        print_str(":");
        // print line number
        let line = location.line();
        let mut digits = [0u8; 10];
        let mut n = 10;
        let mut l = line;
        loop {
            n -= 1;
            digits[n] = b'0' + (l % 10) as u8;
            l /= 10;
            if l == 0 { break; }
        }
        for d in &digits[n..] {
            sbi_putchar(*d);
        }
    }
    print_str(": ");
    // 尝试使用 Display trait 格式化消息
    use core::fmt::Write;
    struct PanicWriter;
    impl Write for PanicWriter {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for b in s.bytes() {
                crate::arch::sbi::console_putchar(b);
            }
            Ok(())
        }
    }
    let _ = core::write!(PanicWriter, "{}", info.message());
    print_str("\x1b[0m\n");

    loop {
        core::hint::spin_loop();
    }
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("alloc error: {:?}", layout);
}

