# RISC-V 陷阱处理汇编代码
#
# 设计：
# TrapContext 保存在内核栈上（每个进程一个，在内核栈顶端）
# sscratch = TrapContext 内核地址（用户态时）
# sscratch = 0（内核态时）
#
# TrapContext 布局：
#   x[0..32]:     offset 0..256
#   sstatus:      offset 256
#   sepc:         offset 264
#   user_satp:    offset 272    (用户进程的页表 satp)
#   kernel_satp:  offset 280    (内核页表 satp，陷入时切换)

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
    # 检查是否来自用户态
    # 如果 sscratch != 0，说明来自用户态（sscratch = TrapContext 内核地址）
    csrr t0, sscratch
    beqz t0, 1f       # 来自内核态，跳转到内核陷阱处理

    # 来自用户态：交换 sp 和 sscratch
    # 执行后：sp = TrapContext 内核地址（内核栈）, sscratch = 用户 sp
    csrrw sp, sscratch, sp

    # 保存用户寄存器到 TrapContext
    sd x1, 8(sp)
    # x2 (sp) 从 sscratch 获取（稍后保存）
    sd x3, 24(sp)
    .set n, 4
    .rept 28
        SAVE_GP %n
        .set n, n+1
    .endr

    # 保存 CSR
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 256(sp)   # sstatus
    sd t1, 264(sp)   # sepc

    # 保存用户 sp（从 sscratch 中获取）
    csrr t0, sscratch
    sd t0, 16(sp)    # x2 = 用户 sp

    # 切换到内核页表（如果配置了）
    ld t2, 280(sp)   # kernel_satp
    beqz t2, .Lno_satp_switch_to_kernel
    csrw satp, t2
    sfence.vma zero, zero
.Lno_satp_switch_to_kernel:

    # 设置 sscratch = 0（表示现在在内核态）
    csrw sscratch, zero

    # 调用 Rust 陷阱处理函数
    mv a0, sp
    call trap_handler

    j __restore

1:  # 内核态陷阱
    # 在内核栈上保存寄存器
    addi sp, sp, -288  # 为 TrapContext (288 字节) 分配空间
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
    sd x2, 16(sp)

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
    addi sp, sp, 288
    sret

__restore:
    # 此时 sp 指向 TrapContext（内核栈上）
    # 恢复 sepc 和 sstatus
    ld t0, 264(sp)   # sepc
    ld t1, 256(sp)   # sstatus
    csrw sepc, t0
    csrw sstatus, t1

    # 切换到用户页表
    ld t2, 272(sp)   # user_satp
    beqz t2, .Lno_satp_switch_to_user
    csrw satp, t2    # 切换到用户页表
    sfence.vma zero, zero
.Lno_satp_switch_to_user:

    # 设置 sscratch = sp（TrapContext 内核地址）
    # 下次陷阱时，__alltraps 会交换 sp 和 sscratch
    csrw sscratch, sp

    # 恢复通用寄存器（除了 sp）
    ld x1, 8(sp)
    ld x3, 24(sp)
    .set n, 4
    .rept 28
        LOAD_GP %n
        .set n, n+1
    .endr

    # 恢复用户 sp（最后恢复）
    ld sp, 16(sp)

    # 返回用户态
    sret
