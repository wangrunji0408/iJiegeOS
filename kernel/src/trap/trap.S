.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

.section .text
.globl __alltraps
.globl __restore
.align 2
__alltraps:
    # sscratch holds kernel sp (top of per-task kernel stack) when entering from U
    # If coming from S (kernel trap), sscratch is 0 (we set so at boot)
    csrrw sp, sscratch, sp
    # sp now points to kernel stack top (&TrapContext), sscratch has user sp

    # allocate TrapContext on kernel stack (sp is already pointing at it, see setup)
    # Actually we pre-allocate TrapContext at top of kernel stack; sp points there.

    # skip x0, save x1
    sd x1, 1*8(sp)
    # x2 (sp) - saved from sscratch below
    # save x3..x31
    .set n, 3
    .rept 29
        SAVE_GP %n
        .set n, n+1
    .endr

    # save sstatus, sepc
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)

    # save user sp (from sscratch)
    csrr t2, sscratch
    sd t2, 2*8(sp)

    # a0 = &TrapContext
    mv a0, sp

    # Load kernel_sp and trap_handler from context
    ld t0, 35*8(sp)   # kernel_sp
    ld t1, 36*8(sp)   # trap_handler
    # switch to kernel stack (top)
    mv sp, t0
    jr t1

.align 2
__restore:
    # a0 = &TrapContext
    mv sp, a0
    # save kernel_sp back into sscratch so the NEXT trap from U finds it
    ld t0, 35*8(sp)
    csrw sscratch, t0
    # restore sstatus, sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # restore x1
    ld x1, 1*8(sp)
    # restore x3..x31
    .set n, 3
    .rept 29
        LOAD_GP %n
        .set n, n+1
    .endr
    # restore sp (user)
    ld sp, 2*8(sp)
    sret
