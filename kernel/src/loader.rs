use crate::mm::address::{VirtAddr, VirtPageNum, PAGE_SIZE};
use crate::mm::memory_set::{MapArea, MapPerm, MapType};
use crate::mm::MemorySet;
use alloc::vec::Vec;
use xmas_elf::program::Type as PhType;

pub const USER_STACK_TOP: usize = 0x4000_0000;
pub const USER_STACK_SIZE: usize = 16 * PAGE_SIZE;

pub struct LoadedElf {
    pub memory: MemorySet,
    pub entry: usize,
    pub stack_top: usize,
    pub program_break: usize,
    pub auxv_phdr: usize,
    pub phnum: usize,
    pub phent: usize,
}

pub fn load_elf(data: &[u8]) -> LoadedElf {
    let elf = xmas_elf::ElfFile::new(data).expect("bad elf");
    let hdr = elf.header;
    assert_eq!(hdr.pt1.magic, [0x7f, b'E', b'L', b'F']);
    // RISC-V machine: xmas-elf 0.9 reports RISC_V as a named variant

    // build kernel identity map + user mappings in a new MemorySet
    let mut ms = kernel_identity_space();

    let mut max_end_va: usize = 0;
    let mut phdr_addr: usize = 0;
    let phnum = hdr.pt2.ph_count() as usize;
    let phent = hdr.pt2.ph_entry_size() as usize;

    for ph in elf.program_iter() {
        if ph.get_type() == Ok(PhType::Load) {
            let start_va = ph.virtual_addr() as usize;
            let end_va = start_va + ph.mem_size() as usize;
            let file_off = ph.offset() as usize;
            let file_sz = ph.file_size() as usize;

            let mut perm = MapPerm::U;
            if ph.flags().is_read() { perm |= MapPerm::R; }
            if ph.flags().is_write() { perm |= MapPerm::W; }
            if ph.flags().is_execute() { perm |= MapPerm::X; }

            let start_pg = VirtAddr(start_va).floor().base().as_usize();
            let end_pg = VirtAddr(end_va).ceil().base().as_usize();
            let mut area = MapArea::new(
                VirtAddr(start_pg),
                VirtAddr(end_pg),
                perm,
                MapType::Framed,
            );
            area.map(&mut ms.page_table);
            // Copy file contents at the correct intra-page offset.
            let seg = &data[file_off..file_off + file_sz];
            copy_segment_into_area(&area, &ms, start_va - start_pg, seg);
            ms.areas.push(area);

            if end_va > max_end_va { max_end_va = end_va; }
            if ph.offset() <= hdr.pt2.ph_offset()
                && hdr.pt2.ph_offset() < ph.offset() + ph.file_size()
            {
                phdr_addr = start_va + (hdr.pt2.ph_offset() - ph.offset()) as usize;
            }
        }
    }

    // Program break (heap start) — round up to page
    let program_break = (max_end_va + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

    // User stack: guard + stack pages at USER_STACK_TOP
    let stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;
    let mut stack_area = MapArea::new(
        VirtAddr(stack_bottom),
        VirtAddr(USER_STACK_TOP),
        MapPerm::R | MapPerm::W | MapPerm::U,
        MapType::Framed,
    );
    stack_area.map(&mut ms.page_table);
    ms.areas.push(stack_area);

    LoadedElf {
        memory: ms,
        entry: hdr.pt2.entry_point() as usize,
        stack_top: USER_STACK_TOP,
        program_break,
        auxv_phdr: phdr_addr,
        phnum,
        phent,
    }
}

/// Create a MemorySet that contains kernel identity mappings so
/// trap handlers can run without switching satp.
pub fn kernel_identity_space() -> MemorySet {
    let mut ms = MemorySet::new_bare();
    extern "C" {
        fn stext(); fn etext(); fn srodata(); fn erodata();
        fn sdata(); fn edata(); fn sbss(); fn ebss();
        fn ekernel();
    }
    let s = |f: unsafe extern "C" fn()| f as *const () as usize;
    ms.push_identity(s(stext), s(etext), MapPerm::R | MapPerm::X);
    ms.push_identity(s(srodata), s(erodata), MapPerm::R);
    ms.push_identity(s(sdata), s(edata), MapPerm::R | MapPerm::W);
    ms.push_identity(s(sbss), s(ebss), MapPerm::R | MapPerm::W);
    ms.push_identity(s(ekernel), 0x80000000 + 512 * 1024 * 1024, MapPerm::R | MapPerm::W);
    ms.push_identity(0x0010_0000, 0x0010_1000, MapPerm::R | MapPerm::W);
    ms.push_identity(0x1000_0000, 0x1000_1000, MapPerm::R | MapPerm::W);
    ms.push_identity(0x1000_1000, 0x1000_9000, MapPerm::R | MapPerm::W);
    ms.push_identity(0x0c00_0000, 0x1000_0000, MapPerm::R | MapPerm::W);
    ms
}

fn copy_segment_into_area(area: &MapArea, ms: &MemorySet, start_off: usize, data: &[u8]) {
    let mut written = 0usize;
    let mut intra = start_off;
    for vpn in area.start_vpn.0..area.end_vpn.0 {
        if written >= data.len() { break; }
        let in_page = PAGE_SIZE - intra;
        let n = core::cmp::min(in_page, data.len() - written);
        let pte = ms.page_table.find_pte(VirtPageNum(vpn)).expect("no pte");
        let base = pte.ppn().base().as_usize();
        unsafe {
            core::ptr::copy_nonoverlapping(
                data[written..written + n].as_ptr(),
                (base + intra) as *mut u8,
                n,
            );
        }
        written += n;
        intra = 0;
    }
}
