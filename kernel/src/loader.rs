use crate::mm::address::{VirtAddr, VirtPageNum, PAGE_SIZE};
use crate::mm::memory_set::{MapArea, MapPerm, MapType};
use crate::mm::MemorySet;
use alloc::collections::BTreeMap;
use xmas_elf::program::Type as PhType;

pub const USER_STACK_TOP: usize = 0x4000_0000;
pub const USER_STACK_SIZE: usize = 128 * PAGE_SIZE;

#[derive(Default)]
pub struct ElfInfo {
    pub entry: usize,
    pub phdr: usize,
    pub phnum: usize,
    pub phent: usize,
    pub max_end_va: usize,
    pub min_base: usize,
    pub base: usize,
}

pub struct LoadedElf {
    pub memory: MemorySet,
    pub main: ElfInfo,
    pub interp: Option<ElfInfo>,
    pub stack_top: usize,
    pub program_break: usize,
}

/// Map an ELF into `ms` at base. If ELF is PIE (EXEC with p_vaddr==0 or DYN),
/// segments are shifted by `base`. Returns ElfInfo.
pub fn map_elf(ms: &mut MemorySet, data: &[u8], base: usize) -> ElfInfo {
    let elf = xmas_elf::ElfFile::new(data).expect("bad elf");
    let hdr = elf.header;
    assert_eq!(hdr.pt1.magic, [0x7f, b'E', b'L', b'F']);

    let mut page_perm: BTreeMap<usize, MapPerm> = BTreeMap::new();
    let mut info = ElfInfo::default();
    info.base = base;
    info.phnum = hdr.pt2.ph_count() as usize;
    info.phent = hdr.pt2.ph_entry_size() as usize;
    let mut min_base: usize = usize::MAX;

    for ph in elf.program_iter() {
        if ph.get_type() == Ok(PhType::Load) {
            let start_va = ph.virtual_addr() as usize + base;
            let end_va = start_va + ph.mem_size() as usize;
            let mut perm = MapPerm::U;
            if ph.flags().is_read() { perm |= MapPerm::R; }
            if ph.flags().is_write() { perm |= MapPerm::W; }
            if ph.flags().is_execute() { perm |= MapPerm::X; }

            let start_pg = VirtAddr(start_va).floor().0;
            let end_pg = VirtAddr(end_va).ceil().0;
            for vpn in start_pg..end_pg {
                let entry = page_perm.entry(vpn).or_insert(MapPerm::U);
                *entry |= perm;
            }
            if end_va > info.max_end_va { info.max_end_va = end_va; }
            if start_va < min_base { min_base = start_va; }
            if ph.offset() <= hdr.pt2.ph_offset()
                && hdr.pt2.ph_offset() < ph.offset() + ph.file_size()
            {
                info.phdr = start_va + (hdr.pt2.ph_offset() - ph.offset()) as usize;
            }
        }
    }
    info.min_base = if min_base == usize::MAX { base } else { min_base };
    info.entry = hdr.pt2.entry_point() as usize + base;
    crate::println!("[load] map_elf: {} pages, base={:#x}, entry={:#x}", page_perm.len(), base, info.entry);

    use crate::mm::frame::alloc as alloc_frame;
    let mut frames: BTreeMap<usize, crate::mm::frame::FrameTracker> = BTreeMap::new();
    for (&vpn, &perm) in &page_perm {
        // Skip if already mapped (shouldn't happen for a single ELF)
        if ms.page_table.find_pte(VirtPageNum(vpn)).is_some() { continue; }
        let f = alloc_frame().expect("no frame");
        ms.page_table.map(VirtPageNum(vpn), f.ppn, perm.into());
        frames.insert(vpn, f);
    }

    for ph in elf.program_iter() {
        if ph.get_type() == Ok(PhType::Load) {
            let start_va = ph.virtual_addr() as usize + base;
            let file_off = ph.offset() as usize;
            let file_sz = ph.file_size() as usize;
            let src = &data[file_off..file_off + file_sz];

            let mut va = start_va;
            let mut written = 0usize;
            while written < src.len() {
                let page_va = va & !(PAGE_SIZE - 1);
                let intra = va - page_va;
                let vpn = page_va / PAGE_SIZE;
                let ppn = if let Some(f) = frames.get(&vpn) {
                    f.ppn
                } else {
                    // Already mapped by a previous ELF or earlier PH
                    let pte = ms.page_table.find_pte(VirtPageNum(vpn)).expect("page not tracked");
                    pte.ppn()
                };
                let dst_page = ppn.get_bytes_array();
                let n = core::cmp::min(PAGE_SIZE - intra, src.len() - written);
                dst_page[intra..intra + n].copy_from_slice(&src[written..written + n]);
                va += n;
                written += n;
            }
        }
    }

    // Stash frame ownership in the MemorySet
    let mut hold_area = MapArea::new(VirtAddr(0), VirtAddr(0), MapPerm::U, MapType::Framed);
    for (vpn, f) in frames {
        hold_area.frames.insert(VirtPageNum(vpn), f);
    }
    ms.areas.push(hold_area);

    info
}

pub fn load_program(main_data: &[u8], interp_data: Option<&[u8]>) -> LoadedElf {
    let mut ms = kernel_identity_space();
    crate::println!("[load] kernel_identity_space built");

    let is_pie = xmas_elf::ElfFile::new(main_data).unwrap().header.pt2.type_().as_type()
        == xmas_elf::header::Type::SharedObject;
    crate::println!("[load] main is_pie={}", is_pie);

    let main_base = if is_pie { 0x1000_0000 } else { 0 };
    let main = map_elf(&mut ms, main_data, main_base);
    crate::println!("[load] main mapped entry={:#x}", main.entry);

    let interp = interp_data.map(|idata| {
        let interp_base = 0x2000_0000;
        let r = map_elf(&mut ms, idata, interp_base);
        crate::println!("[load] interp mapped entry={:#x}", r.entry);
        r
    });

    let program_break = (main.max_end_va + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

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
        main,
        interp,
        stack_top: USER_STACK_TOP,
        program_break,
    }
}

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
