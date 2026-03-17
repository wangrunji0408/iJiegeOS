/// RISC-V Sv39 虚拟内存相关

/// 启用 Sv39 虚拟内存
pub fn enable_mmu(satp: usize) {
    unsafe {
        riscv::register::satp::write(satp);
        core::arch::asm!("sfence.vma");
    }
}

/// 刷新 TLB
pub fn flush_tlb() {
    unsafe {
        core::arch::asm!("sfence.vma");
    }
}

/// 当前 satp 值
pub fn current_satp() -> usize {
    riscv::register::satp::read().bits()
}
