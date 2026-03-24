mod fd;
pub use fd::FileDescriptor;

use alloc::sync::Arc;
use spin::Mutex;
use crate::mm::*;
use crate::process::*;
use crate::config::*;
use crate::trap::TrapContext;

pub fn init() {
    // File system initialization
}

/// Load the init process - a simple test program
pub fn load_init_process() {
    // Load the test ELF binary compiled from C
    let test_elf = include_bytes!("../../test_hello.elf");
    load_elf_process(test_elf, &["/init"], &["PATH=/bin"]);
}

pub fn load_elf_process(elf_data: &[u8], argv: &[&str], envp: &[&str]) {
    use xmas_elf::ElfFile;
    use xmas_elf::program::{Type, Flags};

    let elf = ElfFile::new(elf_data).expect("Invalid ELF file");
    let elf_header = elf.header;

    println!("[ELF] Loading ELF: entry={:#x}, type={:?}",
        elf_header.pt2.entry_point(),
        elf_header.pt2.type_().as_type());

    let mut proc = Process::new_empty();
    proc.pid = alloc_pid();

    // Allocate kernel stack
    let kernel_stack_frames = KERNEL_STACK_SIZE / PAGE_SIZE;
    let kernel_stack = crate::mm::frame_alloc_contiguous(kernel_stack_frames)
        .expect("Failed to allocate kernel stack");
    let kernel_stack_bottom = kernel_stack[0].ppn.addr().0;
    let kernel_stack_top = kernel_stack_bottom + KERNEL_STACK_SIZE;
    proc.kernel_stack = kernel_stack_bottom;
    // Leak the frames (they are managed by process lifetime)
    core::mem::forget(kernel_stack);

    // Map kernel space into user page table (identity mapping for kernel)
    {
        let kernel_space = KERNEL_SPACE.lock();
        // Copy kernel mappings
        let kernel_root = kernel_space.page_table.root_ppn();
        let user_root = proc.memory_set.page_table.root_ppn();

        // Copy the upper half entries (kernel space)
        let kernel_entries = kernel_root.as_pte_array();
        let user_entries = user_root.as_pte_array();
        for i in 256..512 {
            user_entries[i] = kernel_entries[i];
        }
        // Also copy lower entries that map kernel identity
        for i in 0..256 {
            if kernel_entries[i].is_valid() {
                user_entries[i] = kernel_entries[i];
            }
        }
    }

    let mut max_end_va = 0usize;

    // Load ELF segments
    for ph in elf.program_iter() {
        if ph.get_type().unwrap() == Type::Load {
            let start_va = ph.virtual_addr() as usize;
            let end_va = start_va + ph.mem_size() as usize;
            let offset = ph.offset() as usize;
            let file_size = ph.file_size() as usize;

            let mut perm = PTEFlags::U;
            let flags = ph.flags();
            if flags.is_read() {
                perm |= PTEFlags::R;
            }
            if flags.is_write() {
                perm |= PTEFlags::W;
            }
            if flags.is_execute() {
                perm |= PTEFlags::X;
            }

            println!("[ELF] LOAD segment: [{:#x}, {:#x}) flags={:?}", start_va, end_va, perm);

            // Map pages
            let start_vpn = VirtAddr(start_va).floor();
            let end_vpn = VirtAddr(end_va).ceil();

            for vpn_val in start_vpn.0..end_vpn.0 {
                let vpn = VirtPageNum(vpn_val);
                let frame = crate::mm::frame_alloc().expect("Failed to allocate frame for ELF");
                let ppn = frame.ppn;
                proc.memory_set.page_table.map(vpn, ppn, perm);
                core::mem::forget(frame); // Process manages the lifetime
            }

            // Copy data
            if file_size > 0 {
                let data = &elf_data[offset..offset + file_size];
                let mut copied = 0;
                let page_offset = start_va & (PAGE_SIZE - 1);

                for vpn_val in start_vpn.0..end_vpn.0 {
                    let vpn = VirtPageNum(vpn_val);
                    let pte = proc.memory_set.page_table.translate(vpn).unwrap();
                    let dst_page = pte.ppn().as_bytes_mut();

                    let dst_start = if vpn_val == start_vpn.0 { page_offset } else { 0 };
                    let copy_len = core::cmp::min(PAGE_SIZE - dst_start, file_size - copied);
                    if copied < file_size {
                        let actual_copy = core::cmp::min(copy_len, file_size - copied);
                        dst_page[dst_start..dst_start + actual_copy]
                            .copy_from_slice(&data[copied..copied + actual_copy]);
                        copied += actual_copy;
                    }
                }
            }

            if end_va > max_end_va {
                max_end_va = end_va;
            }
        }
    }

    // Set up heap (starts after the last segment, page-aligned)
    let heap_start = (max_end_va + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    proc.brk = heap_start;
    proc.brk_start = heap_start;
    proc.heap_bottom = heap_start;

    // Set up user stack (at a high address)
    let user_stack_top = 0x7fff_f000usize;
    let user_stack_bottom = user_stack_top - USER_STACK_SIZE;
    for vpn_val in (user_stack_bottom / PAGE_SIZE)..(user_stack_top / PAGE_SIZE) {
        let vpn = VirtPageNum(vpn_val);
        let frame = crate::mm::frame_alloc().expect("Failed to allocate frame for stack");
        let ppn = frame.ppn;
        proc.memory_set.page_table.map(vpn, ppn, PTEFlags::R | PTEFlags::W | PTEFlags::U);
        core::mem::forget(frame);
    }

    // Set up auxiliary vectors and arguments on stack
    let mut sp = user_stack_top;

    // For now, simple setup: just set entry point and stack pointer
    let entry_point = elf_header.pt2.entry_point() as usize;

    // Build the initial stack layout for Linux:
    // [top of stack]
    // environment strings
    // argument strings
    // padding for alignment
    // auxv (AT_NULL terminator)
    // envp[n] = NULL
    // envp[0..n-1]
    // argv[n] = NULL
    // argv[0..n-1]
    // argc
    // [sp points here]

    // Write strings to stack first
    let mut env_ptrs = alloc::vec::Vec::new();
    let mut arg_ptrs = alloc::vec::Vec::new();

    // Write environment strings
    for env in envp.iter().rev() {
        let bytes = env.as_bytes();
        let len = if bytes.last() == Some(&0) { bytes.len() - 1 } else { bytes.len() };
        sp -= len + 1; // +1 for null terminator
        write_user_bytes(&proc.memory_set.page_table, sp, &bytes[..len]);
        write_user_bytes(&proc.memory_set.page_table, sp + len, &[0]);
        env_ptrs.push(sp);
    }
    env_ptrs.reverse();

    // Write argument strings
    for arg in argv.iter().rev() {
        let bytes = arg.as_bytes();
        let len = if bytes.last() == Some(&0) { bytes.len() - 1 } else { bytes.len() };
        sp -= len + 1;
        write_user_bytes(&proc.memory_set.page_table, sp, &bytes[..len]);
        write_user_bytes(&proc.memory_set.page_table, sp + len, &[0]);
        arg_ptrs.push(sp);
    }
    arg_ptrs.reverse();

    // Align sp to 16 bytes
    sp &= !0xF;

    // Auxiliary vectors
    let auxv: [(usize, usize); 5] = [
        (6, PAGE_SIZE),      // AT_PAGESZ
        (25, 0),             // AT_RANDOM (pointer to 16 random bytes, we'll use 0)
        (23, 0),             // AT_SECURE
        (33, 100),           // AT_MINSIGSTKSZ (dummy)
        (0, 0),              // AT_NULL
    ];

    // Push aux vectors
    for &(key, val) in auxv.iter().rev() {
        sp -= 8;
        write_user_usize(&proc.memory_set.page_table, sp, val);
        sp -= 8;
        write_user_usize(&proc.memory_set.page_table, sp, key);
    }

    // Push envp NULL terminator
    sp -= 8;
    write_user_usize(&proc.memory_set.page_table, sp, 0);

    // Push envp pointers
    for ptr in env_ptrs.iter().rev() {
        sp -= 8;
        write_user_usize(&proc.memory_set.page_table, sp, *ptr);
    }

    // Push argv NULL terminator
    sp -= 8;
    write_user_usize(&proc.memory_set.page_table, sp, 0);

    // Push argv pointers
    for ptr in arg_ptrs.iter().rev() {
        sp -= 8;
        write_user_usize(&proc.memory_set.page_table, sp, *ptr);
    }

    // Push argc
    sp -= 8;
    write_user_usize(&proc.memory_set.page_table, sp, argv.len());

    println!("[ELF] Entry: {:#x}, SP: {:#x}, PID: {}", entry_point, sp, proc.pid);

    // Set up trap context for returning to user space
    let kernel_satp = KERNEL_SPACE.lock().token();
    proc.trap_cx = TrapContext::app_init_context(
        entry_point,
        sp,
        kernel_satp,
        kernel_stack_top,
        crate::trap::trap_handler as usize,
    );

    // Set up task context so that when we switch to this task,
    // it returns to __restore which will restore the trap context
    extern "C" {
        fn __restore();
    }
    proc.task_cx.ra = trap_return as usize;
    proc.task_cx.sp = kernel_stack_top - core::mem::size_of::<TrapContext>();

    // Copy trap context to kernel stack top
    unsafe {
        let cx_ptr = proc.task_cx.sp as *mut TrapContext;
        *cx_ptr = proc.trap_cx.clone();
    }

    let proc = Arc::new(Mutex::new(proc));
    add_process(proc);
}

fn trap_return() {
    // This function is called when switching to a new process
    // It sets up the user page table and jumps to __restore
    let proc = current_process();
    let p = proc.lock();
    let satp = p.token();
    let kernel_sp = p.kernel_stack + KERNEL_STACK_SIZE;
    let trap_cx = &p.trap_cx;

    // Copy trap context to top of kernel stack
    unsafe {
        let sp = kernel_sp - core::mem::size_of::<TrapContext>();
        let cx_ptr = sp as *mut TrapContext;
        *cx_ptr = trap_cx.clone();

        // Set sscratch to user sp
        core::arch::asm!("csrw sscratch, {}", in(reg) trap_cx.x[2]);
        core::arch::asm!("csrw sstatus, {}", in(reg) trap_cx.sstatus);
        core::arch::asm!("csrw sepc, {}", in(reg) trap_cx.sepc);

        // Switch to user page table
        riscv::register::satp::write(satp);
        core::arch::asm!("sfence.vma");

        // Set sp and restore
        core::arch::asm!(
            "mv sp, {sp}",
            "ld x1, 1*8(sp)",
            "ld x3, 3*8(sp)",
            "ld x5, 5*8(sp)",
            "ld x6, 6*8(sp)",
            "ld x7, 7*8(sp)",
            "ld x8, 8*8(sp)",
            "ld x9, 9*8(sp)",
            "ld x10, 10*8(sp)",
            "ld x11, 11*8(sp)",
            "ld x12, 12*8(sp)",
            "ld x13, 13*8(sp)",
            "ld x14, 14*8(sp)",
            "ld x15, 15*8(sp)",
            "ld x16, 16*8(sp)",
            "ld x17, 17*8(sp)",
            "ld x18, 18*8(sp)",
            "ld x19, 19*8(sp)",
            "ld x20, 20*8(sp)",
            "ld x21, 21*8(sp)",
            "ld x22, 22*8(sp)",
            "ld x23, 23*8(sp)",
            "ld x24, 24*8(sp)",
            "ld x25, 25*8(sp)",
            "ld x26, 26*8(sp)",
            "ld x27, 27*8(sp)",
            "ld x28, 28*8(sp)",
            "ld x29, 29*8(sp)",
            "ld x30, 30*8(sp)",
            "ld x31, 31*8(sp)",
            "addi sp, sp, 34*8",
            "csrrw sp, sscratch, sp",
            "sret",
            sp = in(reg) sp,
            options(noreturn),
        );
    }
}

fn write_user_bytes(page_table: &crate::mm::PageTable, va: usize, data: &[u8]) {
    for (i, &byte) in data.iter().enumerate() {
        let addr = va + i;
        let vpn = VirtPageNum(addr / PAGE_SIZE);
        if let Some(pte) = page_table.translate(vpn) {
            let pa = pte.ppn().addr().0 + (addr & (PAGE_SIZE - 1));
            unsafe {
                *(pa as *mut u8) = byte;
            }
        }
    }
}

fn write_user_usize(page_table: &crate::mm::PageTable, va: usize, value: usize) {
    let bytes = value.to_le_bytes();
    write_user_bytes(page_table, va, &bytes);
}
