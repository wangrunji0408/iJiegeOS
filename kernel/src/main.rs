#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(asm_const)]
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
    // 只让 hart 0 初始化
    if hart_id == 0 {
        console::init();
        println!("\x1b[32m");
        println!("██╗     ██╗     ██╗███████╗ ██████╗ ███████╗");
        println!("██║██╗██║██║     ██║██╔════╝██╔════╝ ██╔════╝");
        println!("╚███╔███╔╝██║    ██║█████╗  ██║  ███╗█████╗  ");
        println!(" ╚██╔╝╚██╔╝██║   ██║██╔══╝  ██║   ██║██╔══╝  ");
        println!("  ╚═╝  ╚═╝ ██║██╗██║███████╗╚██████╔╝███████╗");
        println!("            ╚═╝╚═╝╚══════╝ ╚═════╝ ╚══════╝");
        println!("\x1b[0m");
        println!("iJiege OS v0.1.0 - RISC-V64 OS Kernel in Rust");
        println!("Hart ID: {}, DTB: {:#x}", hart_id, dtb_pa);

        // 初始化内存管理
        mm::init();
        println!("Memory management initialized");

        // 初始化日志系统
        logger::init();

        // 初始化架构相关（陷阱处理等）
        arch::init();
        println!("Architecture initialized");

        // 初始化定时器
        timer::init();

        // 初始化驱动（VirtIO等）
        drivers::init(dtb_pa);
        println!("Drivers initialized");

        // 初始化文件系统
        fs::init();
        println!("Filesystem initialized");

        // 初始化网络栈
        net::init();
        println!("Network initialized");

        // 初始化任务管理
        task::init();
        println!("Task manager initialized");

        KERNEL_STARTED.store(true, Ordering::SeqCst);

        // 运行初始进程（init）
        println!("Starting init process...");
        task::run_first_task();
    } else {
        // 等待主 hart 完成初始化
        while !KERNEL_STARTED.load(Ordering::SeqCst) {
            core::hint::spin_loop();
        }
        // 其他 hart 暂时空转
        loop {
            arch::wait_for_interrupt();
        }
    }

    unreachable!("kernel_main should never return");
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
                println!(
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
        log::set_max_level(LevelFilter::Debug);
    }
}
