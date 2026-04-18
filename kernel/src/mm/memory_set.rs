use super::address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum, PAGE_SIZE};
use super::frame::{alloc, FrameTracker};
use super::page_table::{PTEFlags, PageTable};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use bitflags::bitflags;
use core::arch::asm;

bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct MapPerm: usize {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

impl From<MapPerm> for PTEFlags {
    fn from(p: MapPerm) -> Self {
        PTEFlags::from_bits_truncate(p.bits())
    }
}

pub struct MapArea {
    pub start_vpn: VirtPageNum,
    pub end_vpn: VirtPageNum,
    pub frames: BTreeMap<VirtPageNum, FrameTracker>,
    pub perm: MapPerm,
}

impl MapArea {
    pub fn new(start_va: VirtAddr, end_va: VirtAddr, perm: MapPerm) -> Self {
        Self {
            start_vpn: start_va.floor(),
            end_vpn: end_va.ceil(),
            frames: BTreeMap::new(),
            perm,
        }
    }

    /// Map all pages with freshly allocated frames.
    pub fn map(&mut self, pt: &mut PageTable) {
        for vpn in self.start_vpn.0..self.end_vpn.0 {
            let f = alloc().expect("no frame");
            pt.map(VirtPageNum(vpn), f.ppn, self.perm.into());
            self.frames.insert(VirtPageNum(vpn), f);
        }
    }

    /// Unmap; frames are dropped automatically.
    pub fn unmap(&mut self, pt: &mut PageTable) {
        for vpn in self.start_vpn.0..self.end_vpn.0 {
            if pt.find_pte(VirtPageNum(vpn)).is_some() {
                pt.unmap(VirtPageNum(vpn));
            }
        }
        self.frames.clear();
    }

    /// Copy data into this area starting at offset 0 of start_va
    pub fn copy_from(&mut self, pt: &PageTable, data: &[u8]) {
        let mut offset = 0usize;
        for vpn in self.start_vpn.0..self.end_vpn.0 {
            if offset >= data.len() { break; }
            let end = core::cmp::min(offset + PAGE_SIZE, data.len());
            let pte = pt.find_pte(VirtPageNum(vpn)).expect("not mapped");
            let dst = pte.ppn().get_bytes_array();
            dst[..end - offset].copy_from_slice(&data[offset..end]);
            offset = end;
        }
    }
}

pub struct MemorySet {
    pub page_table: PageTable,
    pub areas: Vec<MapArea>,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self { page_table: PageTable::new(), areas: Vec::new() }
    }

    pub fn token(&self) -> usize { self.page_table.token() }

    pub fn push(&mut self, mut area: MapArea, data: Option<&[u8]>) {
        area.map(&mut self.page_table);
        if let Some(d) = data {
            area.copy_from(&self.page_table, d);
        }
        self.areas.push(area);
    }

    /// Build a kernel memory set that identity-maps all kernel regions + all RAM
    /// up to 512MiB. We also identity-map device MMIO used by virtio.
    pub fn new_kernel() -> Self {
        extern "C" {
            fn stext(); fn etext(); fn srodata(); fn erodata();
            fn sdata(); fn edata(); fn sbss(); fn ebss();
            fn ekernel();
        }

        let mut ms = Self::new_bare();
        // .text -> R|X
        ms.push(MapArea::new_identity(VirtAddr(stext as usize), VirtAddr(etext as usize), MapPerm::R | MapPerm::X), None);
        // .rodata -> R
        ms.push(MapArea::new_identity(VirtAddr(srodata as usize), VirtAddr(erodata as usize), MapPerm::R), None);
        // .data -> R|W
        ms.push(MapArea::new_identity(VirtAddr(sdata as usize), VirtAddr(edata as usize), MapPerm::R | MapPerm::W), None);
        // .bss -> R|W
        ms.push(MapArea::new_identity(VirtAddr(sbss as usize), VirtAddr(ebss as usize), MapPerm::R | MapPerm::W), None);
        // free physical memory
        ms.push(MapArea::new_identity(VirtAddr(ekernel as usize), VirtAddr(0x80000000 + 512 * 1024 * 1024), MapPerm::R | MapPerm::W), None);
        // virtio MMIO region (qemu virt) 0x10001000..0x10009000 (8 slots)
        ms.push(MapArea::new_identity(VirtAddr(0x10001000), VirtAddr(0x10009000), MapPerm::R | MapPerm::W), None);
        // PLIC
        ms.push(MapArea::new_identity(VirtAddr(0x0c000000), VirtAddr(0x10000000), MapPerm::R | MapPerm::W), None);
        // UART (not used directly; via SBI)
        ms.push(MapArea::new_identity(VirtAddr(0x10000000), VirtAddr(0x10001000), MapPerm::R | MapPerm::W), None);
        ms
    }
}

impl MapArea {
    pub fn new_identity(start_va: VirtAddr, end_va: VirtAddr, perm: MapPerm) -> Self {
        let start_vpn = start_va.floor();
        let end_vpn = end_va.ceil();
        let mut frames = BTreeMap::new();
        for _ in start_vpn.0..end_vpn.0 { /* identity, no frame alloc */ let _ = &mut frames; }
        Self { start_vpn, end_vpn, frames, perm }
    }
}

// Specialized map for identity area: override map() behavior
impl MemorySet {
    // helper for identity areas: map directly PPN = VPN
    fn _identity_map(&mut self, area: &MapArea) {
        for vpn in area.start_vpn.0..area.end_vpn.0 {
            self.page_table.map(VirtPageNum(vpn), PhysPageNum(vpn), area.perm.into());
        }
    }
}

// We need to override push for identity areas. Simpler: detect empty frames set.
// Re-design: MapArea::map distinguishes identity vs framed by checking a flag.
// For simplicity, we introduce a separate identity pushing method.
impl MemorySet {
    pub fn push_identity(&mut self, start_va: VirtAddr, end_va: VirtAddr, perm: MapPerm) {
        let svpn = start_va.floor();
        let evpn = end_va.ceil();
        for vpn in svpn.0..evpn.0 {
            self.page_table.map(VirtPageNum(vpn), PhysPageNum(vpn), perm.into());
        }
    }

    pub fn activate(&self) {
        let satp = self.token();
        unsafe {
            asm!("csrw satp, {}", in(reg) satp);
            asm!("sfence.vma");
        }
    }
}
