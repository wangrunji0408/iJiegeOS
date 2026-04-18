/// Raw Sstatus manipulation — avoiding dependence on riscv-crate API stability.

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Sstatus {
    pub bits: usize,
}

pub const SSTATUS_SIE: usize = 1 << 1;
pub const SSTATUS_SPIE: usize = 1 << 5;
pub const SSTATUS_SPP: usize = 1 << 8;
pub const SSTATUS_SUM: usize = 1 << 18;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapContext {
    pub x: [usize; 32],
    pub sstatus: usize,
    pub sepc: usize,
    pub kernel_satp: usize,
    pub kernel_sp: usize,
    pub trap_handler: usize,
}

impl TrapContext {
    pub fn app_init(entry: usize, sp: usize, kernel_sp: usize) -> Self {
        // build sstatus:
        //  - SPP = 0 (return to U-mode)
        //  - SPIE = 1 (enable interrupts after sret)
        //  - SUM = 1 (allow S-mode to access U pages)
        let sstatus = SSTATUS_SPIE | SSTATUS_SUM;
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp: 0,
            kernel_sp,
            trap_handler: super::trap_handler as usize,
        };
        cx.x[2] = sp;
        cx
    }
    pub fn set_sp(&mut self, sp: usize) { self.x[2] = sp; }
    pub fn set_entry(&mut self, entry: usize) { self.sepc = entry; }
    pub fn set_arg(&mut self, idx: usize, v: usize) { self.x[10 + idx] = v; }
}

pub fn trap_return(cx_addr: usize) -> ! {
    extern "C" { fn __restore(cx_addr: usize); }
    unsafe { __restore(cx_addr); }
    unreachable!()
}
