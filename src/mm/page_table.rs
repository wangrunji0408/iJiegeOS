use alloc::vec;
use alloc::vec::Vec;
use bitflags::bitflags;
use super::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum, frame_alloc, FrameTracker};
use crate::config::PAGE_SIZE;

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct PTEFlags: u16 {
        const V = 1 << 0;  // Valid
        const R = 1 << 1;  // Read
        const W = 1 << 2;  // Write
        const X = 1 << 3;  // Execute
        const U = 1 << 4;  // User
        const G = 1 << 5;  // Global
        const A = 1 << 6;  // Accessed
        const D = 1 << 7;  // Dirty
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        Self {
            bits: (ppn.0 << 10) | flags.bits() as usize,
        }
    }
    pub fn empty() -> Self {
        Self { bits: 0 }
    }
    pub fn ppn(&self) -> PhysPageNum {
        PhysPageNum((self.bits >> 10) & ((1usize << 44) - 1))
    }
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits_truncate(self.bits as u16)
    }
    pub fn is_valid(&self) -> bool {
        self.flags().contains(PTEFlags::V)
    }
    pub fn is_leaf(&self) -> bool {
        let flags = self.flags();
        flags.contains(PTEFlags::V)
            && (flags.contains(PTEFlags::R) || flags.contains(PTEFlags::X))
    }
    pub fn readable(&self) -> bool {
        self.flags().contains(PTEFlags::R)
    }
    pub fn writable(&self) -> bool {
        self.flags().contains(PTEFlags::W)
    }
    pub fn executable(&self) -> bool {
        self.flags().contains(PTEFlags::X)
    }
    pub fn user(&self) -> bool {
        self.flags().contains(PTEFlags::U)
    }
}

/// Sv39 page table
pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().expect("Failed to allocate frame for page table");
        let ppn = frame.ppn;
        Self {
            root_ppn: ppn,
            frames: vec![frame],
        }
    }

    /// Create a page table from an existing satp value (for reading user space)
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    pub fn root_ppn(&self) -> PhysPageNum {
        self.root_ppn
    }

    /// Generate satp value for Sv39
    pub fn token(&self) -> usize {
        (8usize << 60) | self.root_ppn.0
    }

    /// Find the PTE for the given VPN, creating intermediate tables if create=true
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let indices = vpn.indices();
        let mut ppn = self.root_ppn;
        for (i, &idx) in indices.iter().enumerate() {
            let pte = &mut ppn.as_pte_array()[idx];
            if i == 2 {
                return Some(pte);
            }
            if !pte.is_valid() {
                let frame = frame_alloc().expect("Failed to allocate frame for page table");
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                ppn = frame.ppn;
                self.frames.push(frame);
            } else {
                ppn = pte.ppn();
            }
        }
        None
    }

    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let indices = vpn.indices();
        let mut ppn = self.root_ppn;
        for (i, &idx) in indices.iter().enumerate() {
            let pte = &mut ppn.as_pte_array()[idx];
            if i == 2 {
                return Some(pte);
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        None
    }

    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        if pte.is_valid() {
            panic!("vpn {:?} is already mapped, pte={:#x}", vpn, pte.bits);
        }
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V | PTEFlags::A | PTEFlags::D);

        // Verify by reading back through find_pte
        let check = self.find_pte(vpn);
        if let Some(cpte) = check {
            if cpte.ppn().0 != ppn.0 {
                crate::println!("[PT] BUG: map VPN={:?} PPN={:#x} but find_pte got PPN={:#x}",
                    vpn, ppn.0, cpte.ppn().0);
                // Debug: trace the path
                let indices = vpn.indices();
                crate::println!("[PT]   indices: {:?}", indices);
                let root = self.root_ppn.as_pte_array();
                crate::println!("[PT]   root[{}].bits = {:#x}", indices[0], root[indices[0]].bits);
                if root[indices[0]].is_valid() {
                    let mid = root[indices[0]].ppn().as_pte_array();
                    crate::println!("[PT]   mid[{}].bits = {:#x}", indices[1], mid[indices[1]].bits);
                    if mid[indices[1]].is_valid() {
                        let leaf = mid[indices[1]].ppn().as_pte_array();
                        crate::println!("[PT]   leaf[{}].bits = {:#x}", indices[2], leaf[indices[2]].bits);
                    }
                }
            }
        } else {
            crate::println!("[PT] BUG: map VPN={:?} succeeded but find_pte returned None!", vpn);
        }
    }

    pub fn unmap(&mut self, vpn: VirtPageNum) {
        if let Some(pte) = self.find_pte(vpn) {
            if !pte.is_valid() {
                panic!("vpn {:?} is not mapped", vpn);
            }
            *pte = PageTableEntry::empty();
        }
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn)
            .map(|pte| *pte)
            .filter(|pte| pte.is_valid())
    }

    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        let vpn = va.page_number();
        self.translate(vpn).map(|pte| {
            PhysAddr(pte.ppn().addr().0 + va.page_offset())
        })
    }
}

/// Read a buffer from user space given a page table token
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut result = Vec::new();
    let mut start = ptr as usize;
    let end = start + len;
    while start < end {
        let vpn = VirtAddr(start).page_number();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        let page_start = start - VirtAddr(start).page_offset();
        let page_end = core::cmp::min(page_start + PAGE_SIZE, end);
        let offset = start - page_start;
        let slice_len = page_end - start;
        unsafe {
            result.push(core::slice::from_raw_parts_mut(
                (ppn.addr().0 + offset) as *mut u8,
                slice_len,
            ));
        }
        start = page_end;
    }
    result
}

/// Read a C string from user space
pub fn translated_str(token: usize, ptr: *const u8) -> alloc::string::String {
    let page_table = PageTable::from_token(token);
    let mut string = alloc::string::String::new();
    let mut va = ptr as usize;
    loop {
        let pa = page_table.translate_va(VirtAddr(va)).unwrap();
        let ch: u8 = unsafe { *(pa.0 as *const u8) };
        if ch == 0 {
            break;
        }
        string.push(ch as char);
        va += 1;
    }
    string
}

/// Read a value from user space
pub fn translated_ref<T>(token: usize, ptr: *const T) -> &'static T {
    let page_table = PageTable::from_token(token);
    let pa = page_table.translate_va(VirtAddr(ptr as usize)).unwrap();
    unsafe { (pa.0 as *const T).as_ref().unwrap() }
}

/// Get a mutable reference to a value in user space
pub fn translated_mut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let page_table = PageTable::from_token(token);
    let pa = page_table.translate_va(VirtAddr(ptr as usize)).unwrap();
    unsafe { (pa.0 as *mut T).as_mut().unwrap() }
}
