use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, sscratch, sstatus::{self, SPP}, stval, stvec,
};

global_asm!(include_str!("trap.asm"));

pub const TRAP_CONTEXT_BASE: usize = 0;

/// TrapContext: 保存陷阱时的所有寄存器状态
/// 保存在进程内核栈上（高地址方向）
///
/// 内存布局（由 trap.asm 直接索引）：
///   x[0..32]: offset 0..256     (通用寄存器)
///   sstatus:  offset 256
///   sepc:     offset 264
///   user_satp: offset 272       (用户进程的页表 token)
///   kernel_satp: offset 280     (内核页表 token，用于从用户态陷入时切换)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapContext {
    /// 通用寄存器 x0-x31
    pub x: [usize; 32],
    /// sstatus
    pub sstatus: usize,
    /// sepc (程序计数器)
    pub sepc: usize,
    /// 用户进程页表 token (satp 寄存器值)
    pub user_satp: usize,
    /// 内核页表 token（切换回内核时用）
    pub kernel_satp: usize,
}

impl TrapContext {
    pub fn new(entry: usize, user_sp: usize) -> Self {
        // 构造 sstatus：SPP=User, SPIE=1
        let sstatus_val = {
            let bits = sstatus::read();
            // SPP = 0 (User), SPIE = 1
            let mut val: usize = unsafe { core::mem::transmute(bits) };
            val &= !(1 << 8);  // clear SPP (User)
            val |= 1 << 5;     // set SPIE
            val
        };
        let mut ctx = Self {
            x: [0; 32],
            sstatus: sstatus_val,
            sepc: entry,
            user_satp: 0,      // 由 task 设置
            kernel_satp: 0,    // 由 task 设置
        };
        ctx.x[2] = user_sp;
        ctx
    }

    pub fn set_sp(&mut self, sp: usize) { self.x[2] = sp; }
    pub fn get_sp(&self) -> usize { self.x[2] }

    pub fn syscall_id(&self) -> usize { self.x[17] }
    pub fn syscall_args(&self) -> [usize; 6] {
        [self.x[10], self.x[11], self.x[12], self.x[13], self.x[14], self.x[15]]
    }
    pub fn set_return_value(&mut self, val: usize) { self.x[10] = val; }
    pub fn set_arg(&mut self, i: usize, val: usize) { self.x[10 + i] = val; }
    pub fn get_arg(&self, i: usize) -> usize { self.x[10 + i] }
}

pub fn init() {
    extern "C" { fn __alltraps(); }
    unsafe {
        sscratch::write(0);
        stvec::write(__alltraps as usize, TrapMode::Direct);
        sie::set_stimer();
        sie::set_sext();
    }
    log::info!("trap: stvec={:#x}", __alltraps as usize);
}

// Track 21f60 call sites (sepc at jal instruction in 21f60 callers)
// 21fdc: jal 22522 inside 21f60
const MALLOC_JAL: usize = 0x40021fdc;
// 21f90: jal memset at start of 21f60
const MEMSET_JAL: usize = 0x40021f90;

#[no_mangle]
pub extern "C" fn trap_handler(ctx: &mut TrapContext) {
    let scause = scause::read();
    let stval = stval::read();

    log::debug!("trap: cause={:?}, stval={:#x}, sepc={:#x}", scause.cause(), stval, ctx.sepc);

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            ctx.sepc += 4;
            let syscall_id = ctx.syscall_id();
            let args = ctx.syscall_args();
            log::debug!("syscall: id={}, args={:?}", syscall_id, args);
            let ret = crate::syscall::syscall(syscall_id, args, ctx);
            // 记录 syscall 调用（仅最关键的）
            {
                use core::sync::atomic::{AtomicBool, Ordering};
                static FORK_HAPPENED: AtomicBool = AtomicBool::new(false);
                if syscall_id == 220 { FORK_HAPPENED.store(true, Ordering::Relaxed); }
                let pid = crate::task::current_task().map(|t| t.pid.0).unwrap_or(0);
                if FORK_HAPPENED.load(Ordering::Relaxed) {
                    // fork 之后：记录 pid=2 的关键 syscall
                    let interesting = matches!(syscall_id,
                        242 | 202 |  // accept4, accept
                        64 | 66 |    // write, writev
                        63 | 65 |    // read, readv
                        210 | 206 | 211  // shutdown, sendto, sendmsg
                    );
                    if pid == 2 && interesting {
                        log::warn!("[2]sc{}(fd={})={}", syscall_id, args[0], ret);
                    }
                }
                // 记录 sc258 的调用 PC（sepc 已经 +4，需要减去 4）
                if syscall_id == 258 {
                    log::error!("sc258 called from sepc={:#x} ra={:#x} a0={:#x} a1={:#x}",
                        ctx.sepc - 4, ctx.x[1], ctx.x[10], ctx.x[11]);
                }
            }
            if syscall_id == 222 || syscall_id == 214 || syscall_id == 226 {
                log::debug!("syscall {} ret={:#x}", syscall_id, ret as usize);
            }
            ctx.set_return_value(ret as usize);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            crate::timer::handle_timer_interrupt();
            // 定期打印进程状态用于调试
            {
                use core::sync::atomic::{AtomicU64, Ordering};
                static TICK: AtomicU64 = AtomicU64::new(0);
                let tick = TICK.fetch_add(1, Ordering::Relaxed);
                if tick % 1 == 0 {
                    let pid = crate::task::current_task().map(|t| t.pid.0).unwrap_or(9999);
                    log::error!("timer: pid={} sepc={:#x} sp={:#x} a0={:#x} a1={:#x} a2={:#x} ra={:#x}",
                        pid, ctx.sepc, ctx.x[2], ctx.x[10], ctx.x[11], ctx.x[12], ctx.x[1]);
                }
            }
            crate::task::suspend_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            crate::drivers::handle_external_interrupt();
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault) => {
            if !crate::mm::handle_page_fault(stval, scause.bits()) {
                log::warn!("Store fault: addr={:#x}, sepc={:#x}, ra={:#x}, sp={:#x}, a0={:#x}, a5={:#x}, s6={:#x}",
                    stval, ctx.sepc, ctx.x[1], ctx.x[2], ctx.x[10], ctx.x[15], ctx.x[22]);
                // Print all registers for debugging
                for i in 0..32 {
                    log::warn!("  x[{}]={:#x}", i, ctx.x[i]);
                }
                crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
            }
        }
        Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            if !crate::mm::handle_page_fault(stval, scause.bits()) {
                log::warn!("Load fault: addr={:#x}, sepc={:#x}, ra={:#x}, sp={:#x}, a0={:#x}, a1={:#x}, a2={:#x}, s0={:#x}",
                    stval, ctx.sepc, ctx.x[1], ctx.x[2], ctx.x[10], ctx.x[11], ctx.x[12], ctx.x[8]);
                // Read the ngx_cached_err_log_time (s0 = 0x701014f8)
                let tok = crate::task::current_user_token();
                let s0 = ctx.x[8];
                if s0 != 0 {
                    let p0 = crate::mm::translated_ref(tok, s0 as *const usize);
                    let p8 = crate::mm::translated_ref(tok, (s0 + 8) as *const usize);
                    log::warn!("  *(s0+0)={:#x} (len), *(s0+8)={:#x} (data)", *p0, *p8);
                }
                // Check nginx spin lock at 0x700ff100
                let lock_addr = 0x700ff100usize;
                let lock_val = crate::mm::translated_ref(tok, lock_addr as *const usize);
                log::warn!("  nginx spinlock at {:#x} = {:#x}", lock_addr, *lock_val);
                crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
            }
        }
        Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault) => {
            log::warn!("IPF: addr={:#x}, sepc={:#x}, handling...", stval, ctx.sepc);
            if !crate::mm::handle_page_fault(stval, scause.bits()) {
                log::warn!("Instruction fault: addr={:#x}, sepc={:#x}", stval, ctx.sepc);
                crate::task::current_add_signal(crate::signal::Signal::SIGSEGV);
            } else {
                log::warn!("IPF: handled ok, addr={:#x}", stval);
            }
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
    crate::task::handle_signals();
}

#[no_mangle]
pub extern "C" fn kernel_trap_handler(ctx: &mut TrapContext) {
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            crate::timer::handle_timer_interrupt();
            {
                use core::sync::atomic::{AtomicU64, Ordering};
                static KTICK: AtomicU64 = AtomicU64::new(0);
                let tick = KTICK.fetch_add(1, Ordering::Relaxed);
                if tick % 10 == 0 {
                    let pid = crate::task::current_task().map(|t| t.pid.0).unwrap_or(9999);
                    log::error!("ktimer: pid={} sepc={:#x}", pid, ctx.sepc);
                }
            }
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            crate::drivers::handle_external_interrupt();
        }
        _ => {
            panic!("kernel trap: {:?}, stval={:#x}, sepc={:#x}",
                scause.cause(), stval, ctx.sepc);
        }
    }
}
