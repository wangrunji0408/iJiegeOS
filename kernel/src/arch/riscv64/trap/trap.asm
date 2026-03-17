# RISC-V 陷阱处理汇编代码
#
# 简化设计：内核使用直接映射（恒等映射）
# 用户程序也使用 KERNEL_SPACE 的高端映射 + 用户段
# 陷阱处理器在内核地址空间，不需要 trampoline
#
# 陷阱帧（TrapContext）保存在进程的内核栈上
# sscratch 保存：当在用户态时 = 进程内核栈上 TrapContext 的内核地址
#               当在内核态时 = 0

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

# TrapContext 结构（保存在内核栈上，由汇编访问）:
# x[0..32]: offset 0..256
# sstatus:  offset 256
# sepc:     offset 264

__alltraps:
    # 检查是否来自用户态
    # 如果 sscratch != 0，说明来自用户态（sscratch 保存内核栈指针）
    csrr t0, sscratch
    beqz t0, 1f       # 来自内核态，跳转到内核陷阱处理

    # 来自用户态：交换 sp 和 sscratch
    csrrw sp, sscratch, sp
    # 现在 sp = 内核栈中 TrapContext 的位置, sscratch = 用户 sp

    # 保存用户寄存器到 TrapContext
    sd x1, 8(sp)
    # x2 = sp, 从 sscratch 获取
    sd x3, 24(sp)
    .set n, 4
    .rept 28
        SAVE_GP %n
        .set n, n+1
    .endr

    # 保存 CSR
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 256(sp)
    sd t1, 264(sp)

    # 保存用户 sp
    csrr t0, sscratch
    sd t0, 16(sp)

    # 设置 sscratch = 0（表示现在在内核态）
    csrw sscratch, zero

    # 调用 Rust 陷阱处理函数
    # sp 指向 TrapContext，作为参数传入
    mv a0, sp
    call trap_handler

    j __restore

1:  # 内核态陷阱
    # 在内核栈上保存寄存器（内核陷阱帧）
    addi sp, sp, -272  # 为 TrapContext 分配空间
    sd x1, 8(sp)
    sd x3, 24(sp)
    .set n, 4
    .rept 28
        SAVE_GP %n
        .set n, n+1
    .endr
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 256(sp)
    sd t1, 264(sp)
    sd x2, 16(sp)  # 保存原 sp（陷阱前的值）实际上 sp 已经改了

    mv a0, sp
    call kernel_trap_handler

    # 恢复内核寄存器
    ld t0, 256(sp)
    ld t1, 264(sp)
    csrw sstatus, t0
    csrw sepc, t1
    ld x1, 8(sp)
    ld x3, 24(sp)
    .set n, 4
    .rept 28
        LOAD_GP %n
        .set n, n+1
    .endr
    addi sp, sp, 272
    sret

__restore:
    # a0 = TrapContext 指针（可忽略，sp 已经指向它）
    # 恢复 sepc 和 sstatus
    ld t0, 264(sp)   # sepc
    ld t1, 256(sp)   # sstatus
    csrw sepc, t0
    csrw sstatus, t1

    # 恢复用户 sp 到 sscratch
    ld t0, 16(sp)    # 用户 sp
    csrw sscratch, sp  # sscratch = TrapContext 内核地址
    # 等等！我们需要 sscratch = 内核栈（下次陷阱的目标）
    # 不对，sscratch 在陷阱入口时等于内核栈，我们在 __alltraps 中交换了
    # 正确的做法：sscratch 指向 TrapContext 的内核地址（即 sp 当前值）
    # 下次陷阱时：交换 sp 和 sscratch，sp = TrapContext 内核地址
    # 这样 TrapContext 的位置在内核栈固定处

    # 设置 sscratch = sp（TrapContext 内核地址）用于下次陷阱
    csrw sscratch, sp

    # 恢复通用寄存器
    ld x1, 8(sp)
    ld x3, 24(sp)
    .set n, 4
    .rept 28
        LOAD_GP %n
        .set n, n+1
    .endr

    # 恢复用户 sp
    ld sp, 16(sp)
    sret
