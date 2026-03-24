    .section .text.entry
    .globl _start
_start:
    # a0 = hartid, a1 = dtb pointer
    # Set up stack for boot hart
    la sp, boot_stack_top

    # Jump to Rust code
    call rust_main

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 64  # 256KB boot stack
    .globl boot_stack_top
boot_stack_top:
