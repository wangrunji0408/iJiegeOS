/// Kernel configuration constants

/// Physical memory start
pub const MEMORY_START: usize = 0x8000_0000;

/// Physical memory end (128MB for now)
pub const MEMORY_END: usize = 0x8800_0000;

/// Kernel base address
pub const KERNEL_BASE: usize = 0x8020_0000;

/// Page size (4KB)
pub const PAGE_SIZE: usize = 4096;

/// Page size bits
pub const PAGE_SIZE_BITS: usize = 12;

/// Kernel heap size (16MB)
pub const KERNEL_HEAP_SIZE: usize = 16 * 1024 * 1024;

/// User stack size (8MB)
pub const USER_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Kernel stack size per process (64KB)
pub const KERNEL_STACK_SIZE: usize = 64 * 1024;

/// Trampoline virtual address (top of address space)
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;

/// Trap context virtual address
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;

/// Max number of processes
pub const MAX_PROC: usize = 256;

/// Clock frequency (QEMU virt: 10MHz)
pub const CLOCK_FREQ: usize = 10_000_000;

/// Max number of file descriptors per process
pub const MAX_FD: usize = 1024;

/// User space start address
pub const USER_SPACE_START: usize = 0x1000;

/// User space end address (lower half of Sv39)
pub const USER_SPACE_END: usize = 0x4000_0000_0000;

/// MMIO regions for QEMU virt platform
pub const MMIO: &[(usize, usize)] = &[
    (0x0010_0000, 0x1000),     // VIRT_TEST/FINISHER
    (0x0010_1000, 0x1000),     // RTC
    (0x0C00_0000, 0x40_0000),  // PLIC
    (0x1000_0000, 0x9000),     // VirtIO MMIO
];
