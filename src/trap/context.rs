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
        // Read current sstatus, set SPP to User (bit 8 = 0), SPIE (bit 5 = 1)
        let sstatus: usize;
        unsafe {
            core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
        }
        // Clear SPP (bit 8), set SPIE (bit 5)
        let sstatus = (sstatus & !(1 << 8)) | (1 << 5);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        cx.set_sp(sp);
        cx
    }
}
