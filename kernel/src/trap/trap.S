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
    # Entering from U-mode: sscratch holds the TrapContext pointer
    # Swap so that sp -> &TrapContext, sscratch -> user sp
    csrrw sp, sscratch, sp

    sd x1, 1*8(sp)
    .set n, 3
    .rept 29
        SAVE_GP %n
        .set n, n+1
    .endr

    # Save csrs
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)

    # Save user sp
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # Put TrapContext address back into sscratch so the next trap lands here.
    mv t2, sp
    csrw sscratch, t2

    # a0 = &TrapContext
    mv a0, sp
    # Switch to kernel stack
    ld t0, 35*8(sp)   # kernel_sp
    ld t1, 36*8(sp)   # trap_handler
    mv sp, t0
    jalr t1             # call trap_handler(&mut TrapContext) -> &mut TrapContext
    # After return, a0 still points at the (possibly new) TrapContext
    # fall-through into __restore
    j __restore

.align 2
__restore:
    # a0 = &TrapContext
    mv sp, a0
    # Make sure sscratch still points at cx for next trap
    csrw sscratch, a0

    ld t0, 32*8(sp)   # sstatus
    ld t1, 33*8(sp)   # sepc
    csrw sstatus, t0
    csrw sepc, t1

    ld x1, 1*8(sp)
    .set n, 3
    .rept 29
        LOAD_GP %n
        .set n, n+1
    .endr
    # restore sp (user)
    ld sp, 2*8(sp)
    sret
