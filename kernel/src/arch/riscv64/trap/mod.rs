use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, sscratch, sstatus, stval, stvec,
};

global_asm!(include_str!("trap.asm"));

pub const TRAP_CONTEXT_BASE: usize = 0;  // 不使用固定地址，TrapContext 在内核栈上

/// TrapContext: 保存陷阱时的所有寄存器状态
/// 保存在进程内核栈上（高地址方向）
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapContext {
    /// 通用寄存器 x0-x31
    pub x: [usize; 32],
    /// sstatus
    pub sstatus: usize,
    /// sepc (程序计数器)
    pub sepc: usize,
}

impl TrapContext {
    pub fn new(
        entry: usize,
        user_sp: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        // SPP = User
        sstatus.set_spp(sstatus::SPP::User);
        // 开启用户态浮点
        let mut ctx = Self {
            x: [0; 32],
            sstatus: sstatus.bits(),
            sepc: entry,
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

    pub fn set_arg(&mut self, i: usize, val: usize) {
        self.x[10 + i] = val;
    }

    pub fn get_arg(&self, i: usize) -> usize {
        self.x[10 + i]
    }
}

pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        sscratch::write(0);
        stvec::write(__alltraps as usize, TrapMode::Direct);
        // 开启 S 态定时器中断和外部中断
        sie::set_stimer();
        sie::set_sext();
    }
    log::info!("trap: stvec={:#x}", __alltraps as usize);
}

/// 用户态陷阱处理（从汇编调用）
#[no_mangle]
pub extern "C" fn trap_handler(ctx: &mut TrapContext) {
    let scause = scause::read();
    let stval = stval::read();

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            ctx.sepc += 4;
            let syscall_id = ctx.syscall_id();
            let args = ctx.syscall_args();
            let ret = crate::syscall::syscall(syscall_id, args, ctx);
            ctx.set_return_value(ret as usize);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            crate::timer::handle_timer_interrupt();
            crate::task::suspend_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            crate::drivers::handle_external_interrupt();
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault) => {
            if !crate::mm::handle_page_fault(stval, scause.bits()) {
                log::warn!("Store fault: addr={:#x}, sepc={:#x}", stval, ctx.sepc);
                crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
            }
        }
        Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            if !crate::mm::handle_page_fault(stval, scause.bits()) {
                log::warn!("Load fault: addr={:#x}, sepc={:#x}", stval, ctx.sepc);
                crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
            }
        }
        Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault) => {
            log::warn!("Instruction fault: addr={:#x}, sepc={:#x}", stval, ctx.sepc);
            crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            log::warn!("Illegal instruction: sepc={:#x}", ctx.sepc);
            crate::task::current_add_signal(crate::signal::Signal::SIGILL);
        }
        _ => {
            log::warn!("Unhandled trap: {:?}, stval={:#x}, sepc={:#x}",
                scause.cause(), stval, ctx.sepc);
        }
    }

    // 处理信号
    crate::task::handle_signals();
}

/// 内核态陷阱处理
#[no_mangle]
pub extern "C" fn kernel_trap_handler(ctx: &mut TrapContext) {
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
                "kernel trap: {:?}, stval={:#x}, sepc={:#x}",
                scause.cause(), stval, ctx.sepc
            );
        }
    }
}
