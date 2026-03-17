use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        crate::println!(
            "\x1b[31m[PANIC] at {}:{}: {}\x1b[0m",
            location.file(),
            location.line(),
            info.message()
        );
    } else {
        crate::println!("\x1b[31m[PANIC]: {}\x1b[0m", info.message());
    }
    loop {
        core::hint::spin_loop();
    }
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("alloc error: {:?}", layout);
}
