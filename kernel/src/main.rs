#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

extern crate alloc;

#[macro_use]
mod console;
mod lang_items;
mod arch;
mod mm;
mod task;
mod fs;
mod net;
mod syscall;
mod drivers;
mod timer;
mod sync;
mod signal;
mod loader;

use core::sync::atomic::{AtomicBool, Ordering};

static KERNEL_STARTED: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub fn kernel_main(hart_id: usize, dtb_pa: usize) -> ! {
    if hart_id == 0 {
        console::init();
        println!("\x1b[32miJiege OS v0.1.0 - RISC-V64 Kernel\x1b[0m");
        println!("Hart ID: {}, DTB: {:#x}", hart_id, dtb_pa);

        println!("Initializing memory...");
        mm::init();
        println!("Memory initialized OK");
        logger::init();
        log::info!("Memory initialized");

        println!("Initializing arch...");
        arch::init();
        log::info!("Architecture initialized");

        println!("Initializing timer...");
        timer::init();

        println!("Initializing drivers...");
        drivers::init(dtb_pa);
        log::info!("Drivers initialized");

        println!("Initializing filesystem...");
        fs::init();
        log::info!("Filesystem initialized");

        println!("Initializing network...");
        net::init();
        log::info!("Network initialized");

        println!("Initializing tasks...");
        task::init();
        log::info!("Task manager initialized");

        KERNEL_STARTED.store(true, Ordering::SeqCst);

        log::info!("Starting init process...");
        task::run_first_task();
    } else {
        while !KERNEL_STARTED.load(Ordering::SeqCst) {
            core::hint::spin_loop();
        }
        loop {
            arch::wait_for_interrupt();
        }
    }

    unreachable!()
}

mod logger {
    use log::{Level, LevelFilter, Metadata, Record};

    struct KernelLogger;

    impl log::Log for KernelLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= Level::Debug
        }

        fn log(&self, record: &Record) {
            if self.enabled(record.metadata()) {
                let color = match record.level() {
                    Level::Error => "\x1b[31m",
                    Level::Warn  => "\x1b[33m",
                    Level::Info  => "\x1b[32m",
                    Level::Debug => "\x1b[36m",
                    Level::Trace => "\x1b[37m",
                };
                crate::println!(
                    "{}[{}] {}\x1b[0m",
                    color,
                    record.level(),
                    record.args()
                );
            }
        }

        fn flush(&self) {}
    }

    static LOGGER: KernelLogger = KernelLogger;

    pub fn init() {
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(LevelFilter::Error);
    }
}
