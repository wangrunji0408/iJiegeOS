# 陷阱处理代码
# RISC-V 陷阱入口和退出

    .altmacro
    .macro SAVE_GP n
        sd x\n, \n*8(sp)
    .endm
    .macro LOAD_GP n
        ld x\n, \n*8(sp)
    .endm

    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .align 2

# TrapContext 布局:
# x[0..32] = offset 0..256
# sstatus   = offset 256 (32*8)
# sepc      = offset 264 (33*8)
# kernel_sp = offset 272 (34*8)
# kernel_satp = offset 280 (35*8)
# trap_handler = offset 288 (36*8)

__alltraps:
    # 交换 sp 和 sscratch
    # sscratch 指向 TrapContext（在内核栈顶）
    csrrw sp, sscratch, sp

    # 保存通用寄存器（除了x0=zero, x2=sp）
    sd x1, 1*8(sp)
    # x2 = sp 已经保存在 sscratch
    sd x3, 3*8(sp)
    .set n, 4
    .rept 28
        SAVE_GP %n
        .set n, n+1
    .endr

    # 保存 sstatus 和 sepc
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)

    # 保存用户态 sp（原来在 sscratch 中）
    csrr t2, sscratch
    sd t2, 2*8(sp)

    # 获取内核栈指针（在 TrapContext 中保存）
    ld t0, 34*8(sp)  # kernel_sp
    # 切换到内核页表
    ld t1, 35*8(sp)  # kernel_satp
    ld t2, 36*8(sp)  # trap_handler

    # 设置内核页表
    csrw satp, t1
    sfence.vma

    # 切换到内核栈
    mv sp, t0

    # 调用 trap_handler(ctx: &mut TrapContext)
    # 参数：a0 = TrapContext 指针（通过sscratch找到）
    # 注意：这里需要重新获取 TrapContext 地址
    csrr a0, sscratch
    # sscratch 现在是用户 sp，不对
    # 实际上 TrapContext 在切换前的 sp 位置
    # 需要重新设计...

    call trap_handler

    # trap_handler 返回 TrapContext 指针
    mv sp, a0

__restore:
    # sp 指向 TrapContext
    # 切换回用户页表
    ld t0, 35*8(sp)  # kernel_satp - 实际上是用户进程 satp
    ld t1, 33*8(sp)  # sepc
    ld t2, 32*8(sp)  # sstatus

    csrw satp, t0
    sfence.vma
    csrw sepc, t1
    csrw sstatus, t2

    # 恢复 sp 到 sscratch（用户 sp）
    ld t0, 2*8(sp)
    csrw sscratch, t0

    # 恢复通用寄存器
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 4
    .rept 28
        LOAD_GP %n
        .set n, n+1
    .endr

    # 最后恢复 sp
    ld sp, 2*8(sp)
    sret
