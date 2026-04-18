.section .text.entry
.globl _start
_start:
    # a0 = hartid, a1 = device tree address (from SBI)
    # Only hart 0 proceeds; others park
    bnez a0, .park

    la sp, boot_stack_top
    mv s0, a1            # save dtb

    # clear BSS
    la t0, sbss
    la t1, ebss
1:  bgeu t0, t1, 2f
    sd zero, (t0)
    addi t0, t0, 8
    j 1b
2:
    mv a0, s0
    call rust_main

3:  wfi
    j 3b

.park:
    wfi
    j .park

.section .bss.stack
.globl boot_stack
boot_stack:
    .space 4096 * 16
.globl boot_stack_top
boot_stack_top:
