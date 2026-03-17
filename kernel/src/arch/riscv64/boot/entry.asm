    .section .text.entry
    .globl _start
_start:
    # 设置栈指针
    la sp, boot_stack_top
    # 跳转到Rust入口
    call kernel_main

    # 不应该到达这里
halt:
    wfi
    j halt

    .section .bss.stack
    .globl boot_stack_bottom
boot_stack_bottom:
    .space 4096 * 16   # 64KB 引导栈
    .globl boot_stack_top
boot_stack_top:
