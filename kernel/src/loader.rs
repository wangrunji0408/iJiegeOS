/// ELF 程序加载器
/// 支持加载 RISC-V ELF 可执行文件（包括 PIE）和动态库

use xmas_elf::{
    ElfFile,
    program::{ProgramHeader, Type},
    header,
};
use crate::mm::{
    MemorySet, MapArea, MapType, MapPermission, VirtAddr, PAGE_SIZE,
};

/// 用户栈大小：8MB
pub const USER_STACK_SIZE: usize = 8 * 1024 * 1024;
/// 用户栈顶虚拟地址
pub const USER_STACK_TOP: usize = 0x0000_003f_ffff_f000;
/// PIE 程序的默认基址（musl ld 等动态链接器）
pub const PIE_BASE: usize = 0x4000_0000;

/// ELF 加载结果
pub struct ElfLoadResult {
    pub entry: usize,
    pub phdr_vaddr: usize,   // AT_PHDR
    pub phnum: usize,        // AT_PHNUM
    pub phent: usize,        // AT_PHENT
    pub base: usize,         // AT_BASE (加载基址)
    pub brk_start: usize,
    pub stack_top: usize,
}

/// 加载 ELF 到内存空间（给 new_user 使用）
pub fn load_elf(memory_set: &mut MemorySet, elf_data: &[u8]) -> (MemorySet, usize, usize) {
    let mut ms = core::mem::replace(memory_set, MemorySet::new_bare());
    let result = load_elf_to_memory_set(&mut ms, elf_data, None);
    *memory_set = MemorySet::new_bare();
    (ms, result.stack_top, result.entry)
}

/// 完整 ELF 加载（返回详细信息，用于 execve）
pub fn load_elf_full(memory_set: &mut MemorySet, elf_data: &[u8], base_override: Option<usize>) -> ElfLoadResult {
    load_elf_to_memory_set(memory_set, elf_data, base_override)
}

/// 核心 ELF 加载实现
fn load_elf_to_memory_set(ms: &mut MemorySet, elf_data: &[u8], base_override: Option<usize>) -> ElfLoadResult {
    let elf = ElfFile::new(elf_data).expect("invalid ELF");

    // 验证架构
    assert!(
        elf.header.pt2.machine().as_machine() == header::Machine::RISC_V,
        "ELF is not for RISC-V"
    );

    // 判断是否是 PIE（ET_DYN 类型）
    let is_pie = elf.header.pt2.type_().as_type() == header::Type::SharedObject;

    // 为 PIE 选择基址
    let base = if let Some(b) = base_override {
        b
    } else if is_pie {
        PIE_BASE
    } else {
        0
    };

    let raw_entry = elf.header.pt2.entry_point() as usize;
    let entry = base + raw_entry;

    let mut max_end_va: usize = 0;
    let mut phdr_vaddr: usize = 0;
    let phent = elf.header.pt2.ph_entry_size() as usize;
    let phnum = elf.header.pt2.ph_count() as usize;

    // 加载所有 LOAD 段
    for phdr in elf.program_iter() {
        if phdr.get_type().unwrap() != Type::Load {
            continue;
        }

        let raw_vaddr = phdr.virtual_addr() as usize;
        let va_start = base + raw_vaddr;
        let mem_size = phdr.mem_size() as usize;
        let file_size = phdr.file_size() as usize;
        let file_offset = phdr.offset() as usize;

        if mem_size == 0 {
            continue;
        }

        // 建立权限
        let mut map_perm = MapPermission::U;
        let ph_flags = phdr.flags();
        if ph_flags.is_read()    { map_perm |= MapPermission::R; }
        if ph_flags.is_write()   { map_perm |= MapPermission::W; }
        if ph_flags.is_execute() { map_perm |= MapPermission::X; }

        // 页对齐
        let page_offset = va_start % PAGE_SIZE;
        let va_start_aligned = va_start - page_offset;
        let va_end = va_start + mem_size;
        let va_end_aligned = (va_end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        // 映射区域
        let mut area = MapArea::new(
            VirtAddr::from(va_start_aligned),
            VirtAddr::from(va_end_aligned),
            MapType::Framed,
            map_perm,
            "elf",
        );
        area.map(&mut ms.page_table);

        // 从文件复制数据（file_size 部分）
        if file_size > 0 {
            let src = &elf_data[file_offset..file_offset + file_size];
            copy_to_user_pages(&mut ms, va_start, src);
        }

        // BSS 部分已经是零（MapType::Framed 分配清零的帧）

        ms.areas.push(area);

        if va_end > max_end_va {
            max_end_va = va_end;
        }

        // 找 PHDR 在虚拟地址中的位置（用于 AT_PHDR）
        if phdr_vaddr == 0 && ph_flags.is_read() && !ph_flags.is_execute() {
            // 数据段，不是 phdr
        }
    }

    // 找 PT_PHDR 段来获取 phdr_vaddr
    for phdr in elf.program_iter() {
        if phdr.get_type().unwrap() == Type::Phdr {
            phdr_vaddr = base + phdr.virtual_addr() as usize;
            break;
        }
    }
    // 如果没有 PT_PHDR，用第一个 PT_LOAD + phdr 偏移
    if phdr_vaddr == 0 && phnum > 0 {
        for phdr in elf.program_iter() {
            if phdr.get_type().unwrap() == Type::Load && phdr.offset() == 0 {
                phdr_vaddr = base + elf.header.pt2.ph_offset() as usize;
                break;
            }
        }
    }

    // 设置 heap
    let brk_start = (max_end_va + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    ms.brk_start = brk_start;
    ms.brk = brk_start;

    // 用户栈
    let stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;
    ms.insert_framed_area(
        VirtAddr::from(stack_bottom),
        VirtAddr::from(USER_STACK_TOP),
        MapPermission::R | MapPermission::W | MapPermission::U,
    );

    log::info!("ELF loaded: entry={:#x}, base={:#x}, brk={:#x}, stack={:#x}",
        entry, base, brk_start, USER_STACK_TOP);

    ElfLoadResult {
        entry,
        phdr_vaddr,
        phnum,
        phent,
        base,
        brk_start,
        stack_top: USER_STACK_TOP,
    }
}

/// 将数据写入用户地址空间（跨页处理）
fn copy_to_user_pages(ms: &mut MemorySet, va_start: usize, data: &[u8]) {
    let mut written = 0;
    let total = data.len();

    while written < total {
        let va = va_start + written;
        let vpn = VirtAddr::from(va).floor();
        let page_offset = va % PAGE_SIZE;
        let avail = PAGE_SIZE - page_offset;
        let to_write = (total - written).min(avail);

        if let Some(pte) = ms.page_table.translate(vpn) {
            let page = pte.ppn().get_bytes_array();
            page[page_offset..page_offset + to_write]
                .copy_from_slice(&data[written..written + to_write]);
        } else {
            log::warn!("copy_to_user_pages: no mapping for va={:#x}", va);
        }

        written += to_write;
    }
}
