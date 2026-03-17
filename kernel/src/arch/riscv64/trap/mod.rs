use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, sscratch, sstatus, stval, stvec,
};

global_asm!(include_str!("trap.asm"));

/// TRAP_CONTEXT_BASE: 用户虚拟地址空间中 TrapContext 的固定位置
/// 位于 TRAMPOLINE 下面一页
pub const TRAP_CONTEXT_BASE: usize = 0x3fffff000;  // 接近用户地址空间顶部

/// TrapContext: 保存陷阱时的所有寄存器状态
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapContext {
    /// 通用寄存器 x0-x31
    pub x: [usize; 32],
    /// sstatus
    pub sstatus: usize,
    /// sepc (程序计数器)
    pub sepc: usize,
    /// 内核栈指针
    pub kernel_sp: usize,
    /// 内核页表 token (satp)
    pub kernel_satp: usize,
    /// trap_handler 函数地址
    pub trap_handler: usize,
    /// 用户页表 token (satp) - 放在 kernel_satp 位置，但作用不同
    /// 注意：我们用 kernel_satp 字段存储用户页表，restore时切换用
    pub user_satp: usize,
}

impl TrapContext {
    pub fn new(
        entry: usize,
        user_sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
        user_satp: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        // SPP = User，表示从 S 模式返回到 U 模式
        sstatus.set_spp(sstatus::SPP::User);
        // 允许用户态浮点运算
        let mut ctx = Self {
            x: [0; 32],
            sstatus: sstatus.bits(),
            sepc: entry,
            kernel_sp,
            kernel_satp,
            trap_handler,
            user_satp,
        };
        ctx.x[2] = user_sp;  // sp
        ctx
    }

    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    pub fn get_sp(&self) -> usize {
        self.x[2]
    }

    /// 获取系统调用号 (a7 = x17)
    pub fn syscall_id(&self) -> usize {
        self.x[17]
    }

    /// 获取系统调用参数
    pub fn syscall_args(&self) -> [usize; 6] {
        [self.x[10], self.x[11], self.x[12], self.x[13], self.x[14], self.x[15]]
    }

    /// 设置系统调用返回值 (a0 = x10)
    pub fn set_return_value(&mut self, val: usize) {
        self.x[10] = val;
    }

    /// 设置 a0-a5
    pub fn set_arg(&mut self, i: usize, val: usize) {
        self.x[10 + i] = val;
    }
}

pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        sscratch::write(0);
        stvec::write(__alltraps as usize, TrapMode::Direct);
        // 开启 S 态定时器中断
        sie::set_stimer();
        // 开启 S 态外部中断（PLIC）
        sie::set_sext();
    }
    log::info!("trap: initialized, stvec={:#x}", __alltraps as usize);
}

/// 进入用户态前设置
pub fn enable_user_trap() {
    extern "C" {
        fn __alltraps();
    }
    // Trampoline 在虚拟地址中的位置
    let trampoline_addr = crate::mm::TRAMPOLINE;
    let alltraps_offset = __alltraps as usize - crate::arch::sbi::strampoline_addr();
    let stvec_addr = trampoline_addr + alltraps_offset;
    unsafe {
        stvec::write(stvec_addr, TrapMode::Direct);
    }
}

/// 在内核态设置 stvec 指向内核陷阱处理
pub fn set_kernel_trap() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        stvec::write(kernel_trap_handler as usize, TrapMode::Direct);
    }
}

/// 内核态陷阱处理（简化版，主要处理中断）
#[no_mangle]
pub extern "C" fn kernel_trap_handler() -> ! {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            crate::timer::handle_timer_interrupt();
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            crate::drivers::handle_external_interrupt();
        }
        _ => {
            panic!(
                "kernel trap: cause={:?}, stval={:#x}",
                scause.cause(),
                stval
            );
        }
    }
    loop {}
}

/// 用户陷阱处理入口（从汇编调用）
#[no_mangle]
pub extern "C" fn trap_handler_entry() -> *mut TrapContext {
    // 设置内核态陷阱处理器
    set_kernel_trap();

    // 获取当前进程的 TrapContext
    let ctx = crate::task::current_trap_cx();
    handle_trap(ctx);
    ctx
}

fn handle_trap(ctx: &mut TrapContext) {
    let scause = scause::read();
    let stval = stval::read();

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // 系统调用：跳过 ecall 指令
            ctx.sepc += 4;
            let syscall_id = ctx.syscall_id();
            let args = ctx.syscall_args();
            let ret = crate::syscall::syscall(syscall_id, args);
            ctx.set_return_value(ret as usize);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            crate::timer::handle_timer_interrupt();
            // 时间片到，调度
            crate::task::suspend_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            crate::drivers::handle_external_interrupt();
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault) => {
            let handled = crate::mm::handle_page_fault(stval, scause.bits());
            if !handled {
                log::warn!("Store page fault at {:#x}, sepc={:#x}", stval, ctx.sepc);
                crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
            }
        }
        Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            let handled = crate::mm::handle_page_fault(stval, scause.bits());
            if !handled {
                log::warn!("Load page fault at {:#x}, sepc={:#x}", stval, ctx.sepc);
                crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
            }
        }
        Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault) => {
            log::warn!("Instruction fault at {:#x}, sepc={:#x}", stval, ctx.sepc);
            crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
        }
        _ => {
            log::warn!(
                "Unhandled trap: {:?}, stval={:#x}, sepc={:#x}",
                scause.cause(),
                stval,
                ctx.sepc,
            );
        }
    }

    // 处理信号
    crate::task::handle_signals();
}
