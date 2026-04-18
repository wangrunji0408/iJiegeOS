use super::address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum, PAGE_SIZE};
use super::frame::{alloc, FrameTracker};
use alloc::vec::Vec;
use bitflags::bitflags;

bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct PTEFlags: usize {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        Self { bits: (ppn.0 << 10) | flags.bits() }
    }
    pub fn empty() -> Self { Self { bits: 0 } }
    pub fn ppn(&self) -> PhysPageNum { PhysPageNum((self.bits >> 10) & ((1usize << 44) - 1)) }
    pub fn flags(&self) -> PTEFlags { PTEFlags::from_bits_truncate(self.bits) }
    pub fn is_valid(&self) -> bool { self.flags().contains(PTEFlags::V) }
    pub fn readable(&self) -> bool { self.flags().contains(PTEFlags::R) }
    pub fn writable(&self) -> bool { self.flags().contains(PTEFlags::W) }
    pub fn executable(&self) -> bool { self.flags().contains(PTEFlags::X) }
}

pub struct PageTable {
    pub root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

impl PageTable {
    pub fn new() -> Self {
        let f = alloc().expect("no frame");
        let root = f.ppn;
        Self { root_ppn: root, frames: alloc::vec![f] }
    }

    /// Temporary handle for walking an existing page table (no ownership of frames)
    pub fn from_token(satp: usize) -> Self {
        Self { root_ppn: PhysPageNum(satp & ((1 << 44) - 1)), frames: Vec::new() }
    }

    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut ppn = self.root_ppn;
        for i in 0..3 {
            let pte = &mut ppn.get_pte_array()[idxs[i]];
            if i == 2 { return Some(pte); }
            if !pte.is_valid() {
                let f = alloc()?;
                *pte = PageTableEntry::new(f.ppn, PTEFlags::V);
                self.frames.push(f);
            }
            ppn = pte.ppn();
        }
        unreachable!()
    }

    pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut ppn = self.root_ppn;
        for i in 0..3 {
            let pte = &mut ppn.get_pte_array()[idxs[i]];
            if i == 2 { return if pte.is_valid() { Some(pte) } else { None }; }
            if !pte.is_valid() { return None; }
            ppn = pte.ppn();
        }
        None
    }

    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).expect("no frame for pt");
        assert!(!pte.is_valid(), "vpn {:#x} already mapped", vpn.0);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).expect("not mapped");
        *pte = PageTableEntry::empty();
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }

    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        let pte = self.find_pte(va.floor())?;
        Some(PhysAddr(pte.ppn().0 * PAGE_SIZE + va.page_offset()))
    }

    /// satp value with mode Sv39 (8) and ASID 0
    pub fn token(&self) -> usize { 8usize << 60 | self.root_ppn.0 }
}

/// Read a NUL-terminated C string from user space via this page table.
pub fn read_cstr(pt: &PageTable, va: VirtAddr) -> alloc::string::String {
    let mut s = alloc::string::String::new();
    let mut cur = va.0;
    loop {
        let pa = pt.translate_va(VirtAddr(cur)).expect("bad user va");
        let b = unsafe { core::ptr::read(pa.as_usize() as *const u8) };
        if b == 0 { break; }
        s.push(b as char);
        cur += 1;
    }
    s
}

/// Copy a buffer from user VA space into kernel Vec<u8>.
pub fn read_user_bytes(pt: &PageTable, va: VirtAddr, len: usize) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::with_capacity(len);
    let mut cur = va.0;
    let end = va.0 + len;
    while cur < end {
        let page_end = (cur & !(PAGE_SIZE - 1)) + PAGE_SIZE;
        let chunk_end = core::cmp::min(page_end, end);
        let pa = pt.translate_va(VirtAddr(cur)).expect("bad user va");
        let slice = unsafe { core::slice::from_raw_parts(pa.as_usize() as *const u8, chunk_end - cur) };
        out.extend_from_slice(slice);
        cur = chunk_end;
    }
    out
}

/// Write bytes into user VA space through this page table.
pub fn write_user_bytes(pt: &PageTable, va: VirtAddr, data: &[u8]) {
    let mut cur = va.0;
    let mut idx = 0;
    while idx < data.len() {
        let page_end = (cur & !(PAGE_SIZE - 1)) + PAGE_SIZE;
        let chunk = core::cmp::min(page_end - cur, data.len() - idx);
        let pa = pt.translate_va(VirtAddr(cur)).expect("bad user va");
        let slice = unsafe { core::slice::from_raw_parts_mut(pa.as_usize() as *mut u8, chunk) };
        slice.copy_from_slice(&data[idx..idx + chunk]);
        cur += chunk;
        idx += chunk;
    }
}
