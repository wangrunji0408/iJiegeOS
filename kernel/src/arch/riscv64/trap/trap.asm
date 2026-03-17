# RISC-V 陷阱处理汇编代码
# 设计：使用 sscratch 保存 TrapContext 内核虚拟地址
#       内核和用户地址空间都映射了 trampoline 和 trap_context
#
# TrapContext 布局 (offset):
#   x[0..32]      : 0..256   (寄存器保存区)
#   sstatus       : 256
#   sepc          : 264
#   kernel_sp     : 272
#   kernel_satp   : 280
#   trap_handler  : 288

    .altmacro
    .macro SAVE_GP n
        sd x\n, \n*8(sp)
    .endm
    .macro LOAD_GP n
        ld x\n, \n*8(sp)
    .endm

    .section .text.trampoline
    .globl strampoline
strampoline:

    .globl __alltraps
    .align 2
__alltraps:
    # 进入时：在用户页表，sp=用户栈，sscratch=trap_ctx虚拟地址(TRAP_CONTEXT_BASE)
    csrrw sp, sscratch, sp  # sp = trap_ctx, sscratch = 用户sp

    # 保存通用寄存器 (x0 = 0, 不用保存)
    # x2 = sp, 原值在 sscratch 中
    sd x1,  1*8(sp)
    # x2 后面保存
    sd x3,  3*8(sp)
    .set n, 4
    .rept 28
        SAVE_GP %n
        .set n, n+1
    .endr

    # 保存 sstatus 和 sepc
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)   # sstatus at offset 256
    sd t1, 33*8(sp)   # sepc at offset 264

    # 保存用户 sp
    csrr t2, sscratch
    sd t2, 2*8(sp)    # x2 at offset 16

    # 加载内核信息
    ld sp,   34*8(sp)  # kernel_sp (此时 sp 变成了内核栈)
    # 注意: 上面用 sp 作为 trap_ctx 的基址, 切换 sp 后就不能再用它访问 trap_ctx 了
    # 所以我们在切换 sp 之前, 先读出 kernel_satp 和 trap_handler
    # 重新加载（此时 sp 已经是 kernel_sp, 无法访问旧的 sp）

    # 问题：切换 sp 后需要记住 trap_ctx 的地址
    # 解决：用 a0 临时保存
    # 但是 a0 是参数寄存器，需要传给 trap_handler

    # 正确做法：先保存所有需要的值
    # 让我重写

    .globl __restore
__restore:
    # 占位符
    sret

