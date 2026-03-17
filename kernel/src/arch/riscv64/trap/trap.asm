# RISC-V 陷阱处理汇编代码
# 使用 trampoline 机制，支持切换页表

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
    # 此时 sp = 用户栈, sscratch = 内核TrapContext地址（在内核空间）
    # 但我们在用户页表下，需要访问TrapContext
    # TrapContext 映射在 TRAP_CONTEXT_BASE (虚拟地址)

    # 交换 sp 和 sscratch
    # sscratch 包含 TRAP_CONTEXT_BASE 虚拟地址
    csrrw sp, sscratch, sp

    # 现在 sp = TrapContext (TRAP_CONTEXT_BASE), sscratch = 用户 sp

    # 保存通用寄存器 (x1, x3-x31)
    sd x1, 8(sp)
    # x2 (sp) 保存在后面
    sd x3, 24(sp)
    .set n, 4
    .rept 28
        SAVE_GP %n
        .set n, n+1
    .endr

    # 保存 sstatus 和 sepc
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 256(sp)
    sd t1, 264(sp)

    # 保存用户 sp (原在 sscratch 中)
    csrr t2, sscratch
    sd t2, 16(sp)

    # 加载内核信息 (保存在 TrapContext 后面)
    # kernel_sp: TrapContext+272
    # kernel_satp: TrapContext+280
    # trap_handler: TrapContext+288
    ld t0, 272(sp)   # kernel_sp
    ld t1, 280(sp)   # kernel_satp
    ld t2, 288(sp)   # trap_handler

    # 切换到内核页表
    csrw satp, t1
    sfence.vma

    # 切换到内核栈
    mv sp, t0

    # 调用 Rust trap handler
    # a0 = TRAP_CONTEXT_BASE (用户虚拟地址，内核可以通过当前页表访问？不行)
    # 我们需要传递 TrapContext 的内核物理/虚拟地址
    # 实际上内核页表中也映射了 TRAP_CONTEXT_BASE 对应的物理页
    # trap_handler 的参数是 TrapContext 的内核虚拟地址
    # 这个地址保存在哪里？
    # 需要在 sscratch 中或者某个已知位置
    # 简化：直接把 TRAP_CONTEXT_BASE 对应的物理地址传入
    # 但此时已经切换到内核页表，用内核视角的虚拟地址

    # 实际上最简单的做法：trap_handler 的参数保存在 TrapContext.trap_handler 之后的一个字段
    # 我们在 TrapContext 中保存了 kernel_trap_ctx_pa（内核视角的TrapContext地址）
    # 这个字段在 offset 296

    # 从内核栈顶找到 TrapContext 内核地址（由 trap_init 设置）
    # 简化：使用 sscratch 备份前的值（用户sp）作为进程ID查找，但这太复杂了
    #
    # 更好的方案：trap_handler 接收参数 a0 = trap_ctx_pa（内核物理地址）
    # 这个值在切换前已经放在了 TrapContext.trap_handler 之后

    # 读取 TrapContext 内核虚拟地址（在内核栈底部保存）
    # TrapContext offset 296: trap_ctx_kernel_va
    # 切换到内核页表后还需要找到 TrapContext
    # 方案：使用 tp 寄存器保存 TrapContext 内核地址（在 sscratch 切换前）

    # 实际上最简单：在切换前读取并保存到某个临时位置
    # 但 RISC-V 没有全局可用的临时存储
    #
    # 最终方案：TrapContext 在内核地址空间也有映射
    # 使用固定的内核虚拟地址来访问 TrapContext
    # 每个进程的 TrapContext 在内核中有固定位置
    # 我们用 tp 寄存器保存当前 hart 的 trap_ctx 内核地址

    mv a0, tp   # trap_ctx kernel virtual address
    call t2     # call trap_handler(ctx)

    # 返回：a0 = TrapContext 内核地址
    mv tp, a0
    j __restore

    .globl __restore
__restore:
    # a0 = TrapContext 内核虚拟地址（从 trap_handler 返回）
    # 但我们要切换回用户页表

    # 从 TrapContext 中读取用户 satp
    ld t1, 280(a0)   # user_satp (我们把用户satp存在kernel_satp位置)

    # 切换到用户页表
    csrw satp, t1
    sfence.vma

    # 现在在用户页表下，TrapContext 在 TRAP_CONTEXT_BASE
    li sp, 0x3fffff000   # TRAP_CONTEXT_BASE

    # 恢复 sepc 和 sstatus
    ld t0, 264(sp)   # sepc
    ld t1, 256(sp)   # sstatus
    csrw sepc, t0
    csrw sstatus, t1

    # 将用户 sp 存入 sscratch
    ld t0, 16(sp)
    csrw sscratch, t0

    # 恢复通用寄存器
    ld x1, 8(sp)
    ld x3, 24(sp)
    .set n, 4
    .rept 28
        LOAD_GP %n
        .set n, n+1
    .endr

    # 恢复 sp
    ld sp, 16(sp)
    sret
