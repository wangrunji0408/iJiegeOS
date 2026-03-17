use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, sscratch, sstatus, stval, stvec,
};

global_asm!(include_str!("trap.asm"));

#[repr(C)]
#[derive(Debug, Clone)]
pub struct TrapContext {
    /// 通用寄存器 x0-x31
    pub x: [usize; 32],
    /// 特权级寄存器
    pub sstatus: usize,
    pub sepc: usize,
    /// 内核栈指针（保存在sscratch中）
    pub kernel_sp: usize,
    /// 内核satp（页表基址）
    pub kernel_satp: usize,
    /// trap_handler地址
    pub trap_handler: usize,
}

impl TrapContext {
    pub fn new(entry: usize, sp: usize, kernel_satp: usize, kernel_sp: usize, trap_handler: usize) -> Self {
        let mut sstatus = sstatus::read();
        // 设置 SPP 为 User，即返回用户模式
        sstatus.set_spp(sstatus::SPP::User);
        let mut ctx = Self {
            x: [0; 32],
            sstatus: sstatus.bits(),
            sepc: entry,
            kernel_sp,
            kernel_satp,
            trap_handler,
        };
        // sp = x2
        ctx.x[2] = sp;
        ctx
    }

    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    pub fn set_arg0(&mut self, val: usize) {
        self.x[10] = val;
    }

    pub fn set_arg1(&mut self, val: usize) {
        self.x[11] = val;
    }

    pub fn get_syscall_id(&self) -> usize {
        self.x[17]  // a7
    }

    pub fn get_syscall_args(&self) -> [usize; 6] {
        [
            self.x[10], // a0
            self.x[11], // a1
            self.x[12], // a2
            self.x[13], // a3
            self.x[14], // a4
            self.x[15], // a5
        ]
    }

    pub fn set_return_value(&mut self, val: usize) {
        self.x[10] = val;  // a0
    }
}

pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        sscratch::write(0);
        stvec::write(__alltraps as usize, TrapMode::Direct);
        // 开启定时器中断
        sie::set_stimer();
        // 开启外部中断
        sie::set_sext();
    }
    log::info!("trap: initialized");
}

#[no_mangle]
pub fn trap_handler(ctx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // 系统调用
            ctx.sepc += 4;  // 跳过ecall指令
            let syscall_id = ctx.get_syscall_id();
            let args = ctx.get_syscall_args();
            let ret = crate::syscall::syscall(syscall_id, args);
            ctx.set_return_value(ret as usize);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            // 定时器中断
            crate::timer::handle_timer_interrupt();
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            // 外部中断（PLIC）
            crate::drivers::handle_external_interrupt();
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            // 页面错误
            let handled = crate::mm::handle_page_fault(stval, scause.bits());
            if !handled {
                log::error!(
                    "Unhandled page fault: cause={:?}, addr={:#x}, sepc={:#x}",
                    scause.cause(),
                    stval,
                    ctx.sepc
                );
                // 发送SIGSEGV给当前进程
                crate::task::current_add_signal(crate::task::Signal::SIGSEGV);
            }
        }
        _ => {
            log::warn!(
                "Unsupported trap: cause={:?}, stval={:#x}, sepc={:#x}",
                scause.cause(),
                stval,
                ctx.sepc
            );
        }
    }

    ctx
}
