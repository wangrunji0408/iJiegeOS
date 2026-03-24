use riscv::register::sstatus::{self, Sstatus, SPP};

#[repr(C)]
#[derive(Clone, Debug)]
pub struct TrapContext {
    /// General purpose registers x0-x31
    pub x: [usize; 32],
    /// Supervisor status register
    pub sstatus: usize,
    /// Supervisor exception program counter
    pub sepc: usize,
    /// Kernel satp (page table token)
    pub kernel_satp: usize,
    /// Kernel stack pointer
    pub kernel_sp: usize,
    /// Trap handler address
    pub trap_handler: usize,
}

impl TrapContext {
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        // Enable interrupts in user mode
        sstatus.set_spie(true);
        let mut cx = Self {
            x: [0; 32],
            sstatus: unsafe { core::mem::transmute::<Sstatus, usize>(sstatus) },
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        cx.set_sp(sp);
        cx
    }
}
