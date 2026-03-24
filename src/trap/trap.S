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
    .align 4
__alltraps:
    csrrw sp, sscratch, sp
    # Now sp -> kernel stack, sscratch -> user stack

    # Check if we came from kernel (sscratch == 0)
    # If sscratch is 0, we came from kernel mode
    bnez sp, _save_context
    # Came from kernel, restore sp from sscratch
    csrr sp, sscratch

_save_context:
    # Allocate TrapContext on kernel stack
    addi sp, sp, -34*8

    # Save general purpose registers
    sd x1, 1*8(sp)
    # Skip x2 (sp), save it later
    sd x3, 3*8(sp)
    sd x4, 4*8(sp)
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr

    # Save sstatus and sepc
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)

    # Save user sp from sscratch
    csrr t2, sscratch
    sd t2, 2*8(sp)

    # Set sscratch to 0 to indicate kernel mode
    # csrw sscratch, zero

    # Call trap_handler with context pointer
    mv a0, sp
    call trap_handler

    # a0 now points to TrapContext to restore
    mv sp, a0

__restore:
    # Restore sstatus and sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1

    # Restore user sp to sscratch
    ld t2, 2*8(sp)
    csrw sscratch, t2

    # Restore general purpose registers
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    ld x4, 4*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr

    # Restore sp last (swap with sscratch)
    addi sp, sp, 34*8
    csrrw sp, sscratch, sp

    sret
