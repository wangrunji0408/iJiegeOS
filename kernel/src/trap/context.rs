use riscv::register::sstatus::{self, Sstatus, SPP};

#[repr(C)]
#[derive(Clone)]
pub struct TrapContext {
    /// x0..x31
    pub x: [usize; 32],
    pub sstatus: Sstatus,
    pub sepc: usize,
    /// kernel satp (unused, since kernel mappings live inside user pt)
    pub kernel_satp: usize,
    /// kernel sp for this task
    pub kernel_sp: usize,
    /// trap_handler address
    pub trap_handler: usize,
}

impl TrapContext {
    pub fn app_init(entry: usize, sp: usize, kernel_sp: usize) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        // SPIE must be 1 so sret enables interrupts in user mode
        let mut raw = sstatus.bits();
        raw |= 1 << 5;   // SPIE
        raw &= !(1 << 1); // SIE=0 in S-mode during trap
        // reinterpret via transmute
        let sstatus: Sstatus = unsafe { core::mem::transmute(raw) };
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp: 0,
            kernel_sp,
            trap_handler: super::trap_handler as usize,
        };
        cx.x[2] = sp; // sp
        cx
    }
    pub fn set_sp(&mut self, sp: usize) { self.x[2] = sp; }
    pub fn set_entry(&mut self, entry: usize) { self.sepc = entry; }
    pub fn set_arg(&mut self, idx: usize, v: usize) { self.x[10 + idx] = v; }
}

/// Return to user mode: the context is passed to __restore.
pub fn trap_return(cx_addr: usize) -> ! {
    extern "C" { fn __restore(cx_addr: usize); }
    unsafe { __restore(cx_addr); }
    unreachable!()
}
