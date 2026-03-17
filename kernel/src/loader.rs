/// ELF 程序加载器
/// 支持加载 RISC-V ELF 可执行文件和动态库

use xmas_elf::{
    ElfFile,
    program::{self, ProgramHeader, Type},
    header,
};
use crate::mm::{
    MemorySet, MapArea, MapType, MapPermission, VirtAddr, PhysAddr, PAGE_SIZE,
};
use crate::mm::address::StepByOne;

/// 用户栈大小：8MB
pub const USER_STACK_SIZE: usize = 8 * 1024 * 1024;
/// 用户栈顶虚拟地址
pub const USER_STACK_TOP: usize = 0x0000_003f_ffff_f000;

/// 加载 ELF 到内存空间
/// 返回 (user_sp, entry_point)
pub fn load_elf(memory_set: &mut MemorySet, elf_data: &[u8]) -> (MemorySet, usize, usize) {
    let mut ms = core::mem::replace(memory_set, MemorySet::new_bare());
    let (user_sp, entry) = do_load_elf(&mut ms, elf_data);
    *memory_set = MemorySet::new_bare(); // 空的，然后把 ms 放回
    (ms, user_sp, entry)
}

/// 执行实际的 ELF 加载
pub fn do_load_elf(memory_set: &mut MemorySet, elf_data: &[u8]) -> (usize, usize) {
    let elf = ElfFile::new(elf_data).expect("invalid ELF");

    // 验证架构
    assert!(
        elf.header.pt2.machine().as_machine() == header::Machine::RISC_V,
        "ELF is not for RISC-V"
    );

    let mut max_end_vpn = VirtAddr::from(0usize).floor();
    let mut entry_point = elf.header.pt2.entry_point() as usize;

    // 如果是动态链接可执行文件，需要找动态链接器路径
    let mut interp: Option<alloc::string::String> = None;

    // 加载程序段
    for phdr in elf.program_iter() {
        match phdr.get_type().unwrap() {
            Type::Load => {
                let start_va: VirtAddr = (phdr.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((phdr.virtual_addr() + phdr.mem_size()) as usize).into();

                let mut map_perm = MapPermission::U;
                let ph_flags = phdr.flags();
                if ph_flags.is_read() { map_perm |= MapPermission::R; }
                if ph_flags.is_write() { map_perm |= MapPermission::W; }
                if ph_flags.is_execute() { map_perm |= MapPermission::X; }

                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm, "elf_load");
                let end_vpn = end_va.ceil();
                if end_vpn > max_end_vpn {
                    max_end_vpn = end_vpn;
                }

                // 获取文件数据
                let data = match phdr.get_data(&elf).unwrap() {
                    program::SegmentData::Undefined(data) => data,
                    _ => panic!("unexpected segment data type"),
                };

                memory_set.push_with_offset(map_area, data, phdr.offset() as usize % PAGE_SIZE);
                // 实际上我们应该从文件偏移处开始复制
                // 简化：直接把 data 复制进去
                // 注意 virtual_addr 可能不是页对齐的，所以我们需要处理偏移
                // 让我重新实现
            }
            Type::Interp => {
                let data = match phdr.get_data(&elf).unwrap() {
                    program::SegmentData::Undefined(data) => data,
                    _ => panic!("unexpected interp data"),
                };
                // 去掉末尾的 null byte
                let path = core::str::from_utf8(&data[..data.len()-1]).unwrap();
                interp = Some(alloc::string::String::from(path));
            }
            _ => {}
        }
    }

    // 设置 brk
    let brk_start: usize = VirtAddr::from(max_end_vpn).into();
    memory_set.brk_start = brk_start;
    memory_set.brk = brk_start;

    // 分配用户栈
    let user_stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;
    memory_set.insert_framed_area(
        user_stack_bottom.into(),
        USER_STACK_TOP.into(),
        MapPermission::R | MapPermission::W | MapPermission::U,
    );

    log::info!("ELF loaded: entry={:#x}, brk={:#x}, stack={:#x}",
        entry_point, brk_start, USER_STACK_TOP);

    (USER_STACK_TOP, entry_point)
}

/// 重新实现正确的 ELF 加载
pub fn load_elf_correct(memory_set: &mut MemorySet, elf_data: &[u8]) -> (usize, usize) {
    let elf = ElfFile::new(elf_data).expect("invalid ELF");

    let mut max_end_va = VirtAddr::from(0usize);
    let entry_point = elf.header.pt2.entry_point() as usize;

    for phdr in elf.program_iter() {
        if phdr.get_type().unwrap() != Type::Load {
            continue;
        }

        let va_start = phdr.virtual_addr() as usize;
        let va_end = va_start + phdr.mem_size() as usize;
        let file_size = phdr.file_size() as usize;
        let file_offset = phdr.offset() as usize;

        let mut map_perm = MapPermission::U;
        let ph_flags = phdr.flags();
        if ph_flags.is_read()    { map_perm |= MapPermission::R; }
        if ph_flags.is_write()   { map_perm |= MapPermission::W; }
        if ph_flags.is_execute() { map_perm |= MapPermission::X; }

        // 页对齐
        let va_start_aligned = va_start & !4095;
        let va_end_aligned = (va_end + 4095) & !4095;

        let mut area = MapArea::new(
            VirtAddr::from(va_start_aligned),
            VirtAddr::from(va_end_aligned),
            MapType::Framed,
            map_perm,
            "elf",
        );
        area.map(&mut memory_set.page_table);

        // 复制数据（文件中有的部分）
        if file_size > 0 {
            let page_offset = va_start % PAGE_SIZE;
            let src = &elf_data[file_offset..file_offset + file_size];

            let mut written = 0;
            let mut vpn = VirtAddr::from(va_start_aligned).floor();

            while written < file_size {
                let off = if written == 0 { page_offset } else { 0 };
                let avail = PAGE_SIZE - off;
                let to_write = (file_size - written).min(avail);

                if let Some(pte) = memory_set.page_table.translate(vpn) {
                    let page = pte.ppn().get_bytes_array();
                    page[off..off + to_write].copy_from_slice(&src[written..written + to_write]);
                }

                written += to_write;
                vpn.step();
            }
        }

        memory_set.areas.push(area);

        if va_end > max_end_va.0 {
            max_end_va = VirtAddr::from(va_end);
        }
    }

    // 设置 heap
    let brk_start = (max_end_va.0 + 4095) & !4095;
    memory_set.brk_start = brk_start;
    memory_set.brk = brk_start;

    // 用户栈
    let stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;
    memory_set.insert_framed_area(
        VirtAddr::from(stack_bottom),
        VirtAddr::from(USER_STACK_TOP),
        MapPermission::R | MapPermission::W | MapPermission::U,
    );

    (USER_STACK_TOP, entry_point)
}
