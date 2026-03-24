use core::fmt::{self, Write};
use spin::Mutex;

struct Stdout;

static STDOUT: Mutex<Stdout> = Mutex::new(Stdout);

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            crate::arch::sbi::console_putchar(c);
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    STDOUT.lock().write_fmt(args).unwrap();
}

pub fn init() {
    // Nothing to do for SBI console
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
        $crate::console::_print(format_args!($($arg)*));
        $crate::print!("\n");
    };
}
