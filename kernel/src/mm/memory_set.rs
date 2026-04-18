use super::address::{PhysPageNum, VirtAddr, VirtPageNum, PAGE_SIZE};
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
    fn from(p: MapPerm) -> Self { PTEFlags::from_bits_truncate(p.bits()) }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapType { Identity, Framed }

pub struct MapArea {
    pub start_vpn: VirtPageNum,
    pub end_vpn: VirtPageNum,
    pub frames: BTreeMap<VirtPageNum, FrameTracker>,
    pub perm: MapPerm,
    pub map_type: MapType,
}

impl MapArea {
    pub fn new(start_va: VirtAddr, end_va: VirtAddr, perm: MapPerm, map_type: MapType) -> Self {
        Self {
            start_vpn: start_va.floor(),
            end_vpn: end_va.ceil(),
            frames: BTreeMap::new(),
            perm,
            map_type,
        }
    }

    pub fn map(&mut self, pt: &mut PageTable) {
        for vpn in self.start_vpn.0..self.end_vpn.0 {
            let ppn = match self.map_type {
                MapType::Identity => PhysPageNum(vpn),
                MapType::Framed => {
                    let f = alloc().expect("no frame");
                    let p = f.ppn;
                    self.frames.insert(VirtPageNum(vpn), f);
                    p
                }
            };
            pt.map(VirtPageNum(vpn), ppn, self.perm.into());
        }
    }

    pub fn unmap(&mut self, pt: &mut PageTable) {
        for vpn in self.start_vpn.0..self.end_vpn.0 {
            if pt.find_pte(VirtPageNum(vpn)).is_some() {
                pt.unmap(VirtPageNum(vpn));
            }
        }
        self.frames.clear();
    }

    pub fn copy_from(&self, pt: &PageTable, data: &[u8]) {
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
    pub fn new_bare() -> Self { Self { page_table: PageTable::new(), areas: Vec::new() } }
    pub fn token(&self) -> usize { self.page_table.token() }

    pub fn push(&mut self, mut area: MapArea, data: Option<&[u8]>) {
        area.map(&mut self.page_table);
        if let Some(d) = data { area.copy_from(&self.page_table, d); }
        self.areas.push(area);
    }

    pub fn push_identity(&mut self, start: usize, end: usize, perm: MapPerm) {
        self.push(MapArea::new(VirtAddr(start), VirtAddr(end), perm, MapType::Identity), None);
    }

    pub fn push_framed(&mut self, start: usize, end: usize, perm: MapPerm, data: Option<&[u8]>) -> &mut MapArea {
        self.push(MapArea::new(VirtAddr(start), VirtAddr(end), perm, MapType::Framed), data);
        self.areas.last_mut().unwrap()
    }

    pub fn activate(&self) {
        let satp = self.token();
        unsafe {
            asm!("csrw satp, {}", in(reg) satp);
            asm!("sfence.vma");
        }
    }
}
