use core::fmt::{self, Write};
use spin::Mutex;

struct Uart {
    base: usize,
}

impl Uart {
    const fn new(base: usize) -> Self {
        Self { base }
    }

    fn putchar(&self, c: u8) {
        // 使用SBI putchar
        crate::arch::sbi::console_putchar(c);
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.putchar(byte);
        }
        Ok(())
    }
}

static UART: Mutex<Uart> = Mutex::new(Uart::new(0x10000000));

pub fn init() {
    // SBI UART不需要额外初始化
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    UART.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {
        $crate::console::_print(format_args!("{}\n", format_args!($($arg)*)))
    };
}
