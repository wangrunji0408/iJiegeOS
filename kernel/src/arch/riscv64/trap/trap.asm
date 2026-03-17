# RISC-V 陷阱处理汇编代码
#
# 设计：
# - sscratch 保存 TrapContext 的虚拟地址 (TRAP_CONTEXT_BASE = 0x3fffff000)
# - TrapContext 同时映射在内核虚拟地址空间（通过 PHY_TO_VIRT 或恒等映射）
# - 陷阱时：交换 sp 和 sscratch，用 sp 访问 TrapContext
# - 切换到内核页表前：将内核需要的信息读取到寄存器
#
# TrapContext 结构 (Rust struct TrapContext):
#   x[0..32]:     offset   0 (x0 不保存，但占位)
#   sstatus:      offset 256 (32*8)
#   sepc:         offset 264 (33*8)
#   kernel_sp:    offset 272 (34*8)
#   kernel_satp:  offset 280 (35*8)
#   trap_handler: offset 288 (36*8)

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
    # 入口：用户页表，sp=用户栈，sscratch=TRAP_CONTEXT_BASE虚拟地址
    csrrw sp, sscratch, sp
    # 现在：sp=TrapContext, sscratch=用户sp

    # 保存用户寄存器
    sd x1, 8(sp)
    # x2=sp 后面保存
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

    # 保存用户 sp（来自 sscratch）
    csrr t2, sscratch
    sd t2, 16(sp)

    # 读取内核信息（在切换页表前）
    ld t0, 272(sp)    # kernel_sp
    ld t1, 280(sp)    # kernel_satp
    ld t2, 288(sp)    # trap_handler 函数地址（内核虚拟地址）

    # 存储 TrapContext 的物理/内核地址到一个安全的地方
    # 用 a0 传给 trap_handler，但 a0 已经被保存了
    # 解决方案：把 TrapContext 内核虚拟地址作为参数
    # 这个地址 = kernel_satp 对应页表下，TRAP_CONTEXT_BASE 的物理地址
    # 但我们不需要额外计算，因为内核页表也映射了这个物理页
    # 只需要在调用 trap_handler 时传递正确的地址

    # 为了简化，我们把 TrapContext 的内核虚拟地址存在某个固定位置
    # 或者：使用 tp 寄存器（在内核中不会被修改）
    # 先把 TrapContext 的用户虚拟地址（TRAP_CONTEXT_BASE）存起来
    # 然后在内核中根据页表找到对应的内核地址

    # 最简单的方案：
    # trap_handler(ctx: &mut TrapContext) 的参数是 TrapContext 的某个可访问地址
    # 在内核页表下，我们通过当前进程的 TrapContext 物理地址来访问

    # 将 TrapContext 内核虚拟地址存入 tp（临时用）
    # 内核不用 tp（线程指针），可以借用
    mv tp, sp          # tp = TrapContext 用户虚拟地址 (TRAP_CONTEXT_BASE)

    # 切换页表（到内核页表）
    csrw satp, t1
    sfence.vma

    # 切换到内核栈
    mv sp, t0

    # 此时在内核页表下，tp = TRAP_CONTEXT_BASE (用户虚拟地址，内核不可访问)
    # 需要传递内核可访问的 TrapContext 地址给 trap_handler
    # 方案：trap_handler 使用当前进程的 TCB 中保存的物理地址

    # 简化：直接传 0（让 trap_handler 自己找当前任务的 TrapContext）
    # 或者：把 TrapContext 也映射到内核的某个固定地址
    # 最佳方案：内核中每个进程的 TrapContext 地址固定在内核栈底部之前

    # 实际方案：使用 `current_trap_cx()` 函数通过 current_task 获取
    call trap_handler_entry

    j __restore

    .globl __restore
__restore:
    # a0 = TrapContext 的内核虚拟地址
    # 从 TrapContext 中读取用户 satp
    ld t1, 280(a0)   # user_satp

    # 切换到用户页表
    csrw satp, t1
    sfence.vma

    # 现在在用户页表，sp 仍然是内核栈
    # 将 TrapContext 用户虚拟地址加载到 sp
    li sp, 0x3fffff000   # TRAP_CONTEXT_BASE

    # 恢复 sepc 和 sstatus
    ld t0, 264(sp)
    ld t1, 256(sp)
    csrw sepc, t0
    csrw sstatus, t1

    # 将用户 sp 存入 sscratch（恢复 sscratch 为 TRAP_CONTEXT_BASE）
    # 实际上 sscratch 需要指向 TrapContext
    li t0, 0x3fffff000
    csrw sscratch, t0

    # 恢复通用寄存器
    ld x1, 8(sp)
    ld x3, 24(sp)
    .set n, 4
    .rept 28
        LOAD_GP %n
        .set n, n+1
    .endr

    # 最后恢复 sp（用户栈）
    ld sp, 16(sp)
    sret
