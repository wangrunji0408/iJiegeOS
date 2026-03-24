mod fd;
mod ramfs;

pub use fd::FileDescriptor;
pub use ramfs::{RAMFS, init_ramfs};

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use crate::mm::*;
use crate::process::*;
use crate::config::*;
use crate::trap::TrapContext;

pub fn init() {
    init_ramfs();
}

/// Load the init process - nginx via dynamic linker
pub fn load_init_process() {
    let nginx_path = "/usr/sbin/nginx";
    let argv = &[nginx_path, "-c", "/etc/nginx/nginx.conf"];
    let envp = &["PATH=/usr/sbin:/usr/bin:/bin", "HOME=/"];
    load_elf_from_ramfs(nginx_path, argv, envp);
}

pub fn load_elf_from_ramfs(path: &str, argv: &[&str], envp: &[&str]) {
    let fs = RAMFS.lock();
    let file = fs.get_file(path).expect("ELF file not found in ramfs");
    let elf_data = file.data;
    drop(fs);
    load_elf_process(elf_data, argv, envp);
}

pub fn load_elf_process(elf_data: &[u8], argv: &[&str], envp: &[&str]) {
    use xmas_elf::ElfFile;
    use xmas_elf::program::Type;

    let elf = ElfFile::new(elf_data).expect("Invalid ELF file");
    let elf_header = elf.header;

    let is_pie = elf_header.pt2.type_().as_type() == xmas_elf::header::Type::SharedObject;
    let entry_point = elf_header.pt2.entry_point() as usize;

    println!("[ELF] Loading ELF: entry={:#x}, type={:?}, PIE={}",
        entry_point, elf_header.pt2.type_().as_type(), is_pie);

    let mut proc = Process::new_empty();
    proc.pid = alloc_pid();

    // Allocate kernel stack
    let kernel_stack_frames = KERNEL_STACK_SIZE / PAGE_SIZE;
    let kernel_stack = crate::mm::frame_alloc_contiguous(kernel_stack_frames)
        .expect("Failed to allocate kernel stack");
    let kernel_stack_bottom = kernel_stack[0].ppn.addr().0;
    let kernel_stack_top = kernel_stack_bottom + KERNEL_STACK_SIZE;
    proc.kernel_stack = kernel_stack_bottom;
    core::mem::forget(kernel_stack);

    // Map kernel space into user page table
    {
        let kernel_space = KERNEL_SPACE.lock();
        let kernel_root = kernel_space.page_table.root_ppn();
        let user_root = proc.memory_set.page_table.root_ppn();
        let kernel_entries = kernel_root.as_pte_array();
        let user_entries = user_root.as_pte_array();
        for i in 0..512 {
            if kernel_entries[i].is_valid() {
                user_entries[i] = kernel_entries[i];
            }
        }
    }

    // Base address for PIE executables
    let pie_base: usize = if is_pie { 0x4000_0000 } else { 0 };

    // Load ELF segments
    let mut max_end_va = 0usize;
    let mut phdr_va = 0usize;

    for ph in elf.program_iter() {
        match ph.get_type().unwrap() {
            Type::Load => {
                let start_va = pie_base + ph.virtual_addr() as usize;
                let end_va = start_va + ph.mem_size() as usize;
                let offset = ph.offset() as usize;
                let file_size = ph.file_size() as usize;

                let mut perm = PTEFlags::U;
                let flags = ph.flags();
                if flags.is_read() { perm |= PTEFlags::R; }
                if flags.is_write() { perm |= PTEFlags::W; }
                if flags.is_execute() { perm |= PTEFlags::X; }

                println!("[ELF] LOAD: [{:#x}, {:#x}) perm={:?}", start_va, end_va, perm);
                map_and_copy(&mut proc.memory_set.page_table, start_va, end_va,
                    &elf_data[offset..offset + file_size], perm);

                if end_va > max_end_va { max_end_va = end_va; }
            }
            Type::Phdr => {
                phdr_va = pie_base + ph.virtual_addr() as usize;
            }
            _ => {}
        }
    }

    // Check for PT_INTERP (dynamic linker)
    let mut interp_entry = 0usize;
    let mut interp_base = 0usize;
    let mut has_interp = false;

    for ph in elf.program_iter() {
        if ph.get_type().unwrap() == Type::Interp {
            let offset = ph.offset() as usize;
            let size = ph.file_size() as usize;
            let interp_path_bytes = &elf_data[offset..offset + size];
            let interp_path = core::str::from_utf8(interp_path_bytes)
                .unwrap()
                .trim_end_matches('\0');
            println!("[ELF] Dynamic linker: {}", interp_path);

            // Load the dynamic linker
            let fs = RAMFS.lock();
            if let Some(interp_file) = fs.get_file(interp_path) {
                let interp_data = interp_file.data;
                drop(fs);

                let interp_elf = ElfFile::new(interp_data).expect("Invalid interp ELF");
                interp_base = 0x7000_0000; // Load interp at this base
                interp_entry = interp_base + interp_elf.header.pt2.entry_point() as usize;

                for ph in interp_elf.program_iter() {
                    if ph.get_type().unwrap() == Type::Load {
                        let start_va = interp_base + ph.virtual_addr() as usize;
                        let end_va = start_va + ph.mem_size() as usize;
                        let offset = ph.offset() as usize;
                        let file_size = ph.file_size() as usize;

                        let mut perm = PTEFlags::U;
                        let flags = ph.flags();
                        if flags.is_read() { perm |= PTEFlags::R; }
                        if flags.is_write() { perm |= PTEFlags::W; }
                        if flags.is_execute() { perm |= PTEFlags::X; }

                        println!("[ELF] INTERP LOAD: [{:#x}, {:#x}) perm={:?}", start_va, end_va, perm);
                        map_and_copy(&mut proc.memory_set.page_table, start_va, end_va,
                            &interp_data[offset..offset + file_size], perm);

                        if end_va > max_end_va { max_end_va = end_va; }
                    }
                }
                has_interp = true;
                println!("[ELF] Interp loaded at base={:#x}, entry={:#x}", interp_base, interp_entry);
            } else {
                drop(fs);
                panic!("Dynamic linker {} not found!", interp_path);
            }
        }
    }

    // Set up heap
    let heap_start = (max_end_va + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    proc.brk = heap_start;
    proc.brk_start = heap_start;
    proc.heap_bottom = heap_start;

    // Set up user stack
    let user_stack_top = 0x7fff_f000usize;
    let user_stack_size = 256 * PAGE_SIZE; // 1MB stack
    let user_stack_bottom = user_stack_top - user_stack_size;
    for vpn_val in (user_stack_bottom / PAGE_SIZE)..(user_stack_top / PAGE_SIZE) {
        let vpn = VirtPageNum(vpn_val);
        let frame = crate::mm::frame_alloc().expect("stack OOM");
        let ppn = frame.ppn;
        proc.memory_set.page_table.map(vpn, ppn, PTEFlags::R | PTEFlags::W | PTEFlags::U);
        core::mem::forget(frame);
    }

    // Build the initial stack
    let mut sp = user_stack_top;

    // Write strings first
    let mut env_ptrs = alloc::vec::Vec::new();
    let mut arg_ptrs = alloc::vec::Vec::new();

    // Write 16 bytes of random data for AT_RANDOM
    sp -= 16;
    let random_ptr = sp;
    let random_data = get_random_bytes();
    write_user_bytes(&proc.memory_set.page_table, sp, &random_data);

    for env in envp.iter().rev() {
        let bytes = env.as_bytes();
        sp -= bytes.len() + 1;
        write_user_bytes(&proc.memory_set.page_table, sp, bytes);
        write_user_bytes(&proc.memory_set.page_table, sp + bytes.len(), &[0]);
        env_ptrs.push(sp);
    }
    env_ptrs.reverse();

    for arg in argv.iter().rev() {
        let bytes = arg.as_bytes();
        sp -= bytes.len() + 1;
        write_user_bytes(&proc.memory_set.page_table, sp, bytes);
        write_user_bytes(&proc.memory_set.page_table, sp + bytes.len(), &[0]);
        arg_ptrs.push(sp);
    }
    arg_ptrs.reverse();

    // Align to 16 bytes
    sp &= !0xF;

    // Auxiliary vectors
    let phent = elf_header.pt2.ph_entry_size() as usize;
    let phnum = elf_header.pt2.ph_count() as usize;
    // If we have phdr from program header, use it; otherwise calculate from file layout
    let phdr_addr = if phdr_va != 0 {
        phdr_va
    } else {
        pie_base + elf_header.pt2.ph_offset() as usize
    };

    let actual_entry = pie_base + entry_point;

    let mut auxv: alloc::vec::Vec<(usize, usize)> = alloc::vec![
        (3, phdr_addr),              // AT_PHDR
        (4, phent),                  // AT_PHENT
        (5, phnum),                  // AT_PHNUM
        (6, PAGE_SIZE),              // AT_PAGESZ
        (9, actual_entry),           // AT_ENTRY
        (25, random_ptr),            // AT_RANDOM
        (23, 0),                     // AT_SECURE
        (16, 0),                     // AT_HWCAP
    ];

    if has_interp {
        auxv.push((7, interp_base)); // AT_BASE
    }

    auxv.push((0, 0)); // AT_NULL

    // Push auxv
    for &(key, val) in auxv.iter().rev() {
        sp -= 8;
        write_user_usize(&proc.memory_set.page_table, sp, val);
        sp -= 8;
        write_user_usize(&proc.memory_set.page_table, sp, key);
    }

    // Push envp NULL
    sp -= 8;
    write_user_usize(&proc.memory_set.page_table, sp, 0);
    for ptr in env_ptrs.iter().rev() {
        sp -= 8;
        write_user_usize(&proc.memory_set.page_table, sp, *ptr);
    }

    // Push argv NULL
    sp -= 8;
    write_user_usize(&proc.memory_set.page_table, sp, 0);
    for ptr in arg_ptrs.iter().rev() {
        sp -= 8;
        write_user_usize(&proc.memory_set.page_table, sp, *ptr);
    }

    // Push argc
    sp -= 8;
    write_user_usize(&proc.memory_set.page_table, sp, argv.len());

    // Choose entry point: dynamic linker if present, otherwise the ELF entry
    let real_entry = if has_interp { interp_entry } else { actual_entry };

    println!("[ELF] Final entry: {:#x}, SP: {:#x}, PID: {}", real_entry, sp, proc.pid);

    let kernel_satp = KERNEL_SPACE.lock().token();
    proc.trap_cx = TrapContext::app_init_context(
        real_entry, sp, kernel_satp, kernel_stack_top,
        crate::trap::trap_handler as usize,
    );

    proc.task_cx.ra = trap_return as usize;
    proc.task_cx.sp = kernel_stack_top - core::mem::size_of::<TrapContext>();

    unsafe {
        let cx_ptr = proc.task_cx.sp as *mut TrapContext;
        *cx_ptr = proc.trap_cx.clone();
    }

    let proc = Arc::new(Mutex::new(proc));
    add_process(proc);
}

fn map_and_copy(page_table: &mut PageTable, start_va: usize, end_va: usize, data: &[u8], perm: PTEFlags) {
    let start_vpn = VirtAddr(start_va).floor();
    let end_vpn = VirtAddr(end_va).ceil();
    let total_pages = end_vpn.0 - start_vpn.0;
    let page_offset = start_va & (PAGE_SIZE - 1);
    let mut copied = 0;

    println!("[ELF]   mapping+copying {} pages, data {} bytes", total_pages, data.len());

    for vpn_val in start_vpn.0..end_vpn.0 {
        let vpn = VirtPageNum(vpn_val);
        let ppn = if let Some(pte) = page_table.translate(vpn) {
            pte.ppn()
        } else {
            let frame = crate::mm::frame_alloc().expect("OOM in ELF load");
            let ppn = frame.ppn;
            page_table.map(vpn, ppn, perm | PTEFlags::W);
            core::mem::forget(frame);
            ppn
        };

        // Copy data for this page
        if copied < data.len() {
            let pa = ppn.addr().0;
            let dst_start = if vpn_val == start_vpn.0 { page_offset } else { 0 };
            let copy_len = core::cmp::min(PAGE_SIZE - dst_start, data.len() - copied);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    data[copied..].as_ptr(),
                    (pa + dst_start) as *mut u8,
                    copy_len,
                );
            }
            copied += copy_len;
        }
    }
    println!("[ELF]   done, {} bytes copied", copied);
}

fn trap_return() {
    let proc = current_process();
    let p = proc.lock();
    let satp = p.token();
    let trap_cx = p.trap_cx.clone();
    let kernel_sp = p.kernel_stack + KERNEL_STACK_SIZE;
    drop(p);

    unsafe {
        riscv::register::satp::write(satp);
        core::arch::asm!("sfence.vma");

        let sp = kernel_sp - core::mem::size_of::<TrapContext>();
        let cx_ptr = sp as *mut TrapContext;
        *cx_ptr = trap_cx;

        core::arch::asm!("csrw sscratch, {}", in(reg) (*cx_ptr).x[2]);
        core::arch::asm!("csrw sstatus, {}", in(reg) (*cx_ptr).sstatus);
        core::arch::asm!("csrw sepc, {}", in(reg) (*cx_ptr).sepc);

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

fn write_user_bytes(page_table: &PageTable, va: usize, data: &[u8]) {
    for (i, &byte) in data.iter().enumerate() {
        let addr = va + i;
        let vpn = VirtPageNum(addr / PAGE_SIZE);
        if let Some(pte) = page_table.translate(vpn) {
            let pa = pte.ppn().addr().0 + (addr & (PAGE_SIZE - 1));
            unsafe { *(pa as *mut u8) = byte; }
        }
    }
}

fn write_user_usize(page_table: &PageTable, va: usize, value: usize) {
    write_user_bytes(page_table, va, &value.to_le_bytes());
}

fn get_random_bytes() -> [u8; 16] {
    let mut seed = riscv::register::time::read() as u64;
    let mut bytes = [0u8; 16];
    for b in bytes.iter_mut() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (seed >> 33) as u8;
    }
    bytes
}
