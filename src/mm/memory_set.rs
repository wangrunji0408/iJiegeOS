use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;
use core::ops::Range;
use super::{
    PhysAddr, PhysPageNum, VirtAddr, VirtPageNum,
    PageTable, PTEFlags, FrameTracker, frame_alloc,
};
use crate::config::*;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref KERNEL_SPACE: Arc<Mutex<MemorySet>> = Arc::new(Mutex::new(MemorySet::new_kernel()));
}

/// A mapped memory area
pub struct MapArea {
    pub vpn_range: Range<VirtPageNum>,
    pub frames: BTreeMap<usize, FrameTracker>,
    pub map_type: MapType,
    pub pte_flags: PTEFlags,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    /// Identity mapping (PA = VA)
    Identical,
    /// Framed mapping (allocate new frames)
    Framed,
}

impl MapArea {
    pub fn new(start_va: VirtAddr, end_va: VirtAddr, map_type: MapType, perm: PTEFlags) -> Self {
        let start_vpn = start_va.floor();
        let end_vpn = end_va.ceil();
        Self {
            vpn_range: start_vpn..end_vpn,
            frames: BTreeMap::new(),
            map_type,
            pte_flags: perm,
        }
    }

    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range.start.0..self.vpn_range.end.0 {
            let vpn = VirtPageNum(vpn);
            let ppn = match self.map_type {
                MapType::Identical => PhysPageNum(vpn.0),
                MapType::Framed => {
                    let frame = frame_alloc().expect("Failed to allocate frame");
                    let ppn = frame.ppn;
                    self.frames.insert(vpn.0, frame);
                    ppn
                }
            };
            page_table.map(vpn, ppn, self.pte_flags);
        }
    }

    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range.start.0..self.vpn_range.end.0 {
            let vpn = VirtPageNum(vpn);
            page_table.unmap(vpn);
            if self.map_type == MapType::Framed {
                self.frames.remove(&vpn.0);
            }
        }
    }

    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        let mut start = 0usize;
        let mut vpn = self.vpn_range.start.0;
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(VirtPageNum(vpn))
                .unwrap()
                .ppn()
                .as_bytes_mut()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            vpn += 1;
        }
    }
}

/// A set of mapped memory areas
pub struct MemorySet {
    pub page_table: PageTable,
    pub areas: Vec<MapArea>,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }

    pub fn token(&self) -> usize {
        self.page_table.token()
    }

    pub fn push(&mut self, mut area: MapArea, data: Option<&[u8]>) {
        area.map(&mut self.page_table);
        if let Some(data) = data {
            area.copy_data(&mut self.page_table, data);
        }
        self.areas.push(area);
    }

    /// Insert a framed area and map it
    pub fn insert_framed_area(&mut self, start_va: VirtAddr, end_va: VirtAddr, perm: PTEFlags) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, perm),
            None,
        );
    }

    /// Map a single page
    pub fn map_one(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        self.page_table.map(vpn, ppn, flags);
    }

    /// Create kernel address space
    fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // Map kernel sections
        extern "C" {
            fn stext();
            fn etext();
            fn srodata();
            fn erodata();
            fn sdata();
            fn edata();
            fn sbss();
            fn ebss();
            fn ekernel();
        }
        println!("[MM] Mapping .text [{:#x}, {:#x})", stext as usize, etext as usize);
        memory_set.push(
            MapArea::new(
                VirtAddr(stext as usize),
                VirtAddr(etext as usize),
                MapType::Identical,
                PTEFlags::R | PTEFlags::X,
            ),
            None,
        );
        println!("[MM] Mapping .rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        memory_set.push(
            MapArea::new(
                VirtAddr(srodata as usize),
                VirtAddr(erodata as usize),
                MapType::Identical,
                PTEFlags::R,
            ),
            None,
        );
        println!("[MM] Mapping .data [{:#x}, {:#x})", sdata as usize, edata as usize);
        memory_set.push(
            MapArea::new(
                VirtAddr(sdata as usize),
                VirtAddr(edata as usize),
                MapType::Identical,
                PTEFlags::R | PTEFlags::W,
            ),
            None,
        );
        println!("[MM] Mapping .bss [{:#x}, {:#x})", sbss as usize, ebss as usize);
        memory_set.push(
            MapArea::new(
                VirtAddr(sbss as usize),
                VirtAddr(ebss as usize),
                MapType::Identical,
                PTEFlags::R | PTEFlags::W,
            ),
            None,
        );
        // Map remaining physical memory
        println!("[MM] Mapping physical memory [{:#x}, {:#x})", ekernel as usize, MEMORY_END);
        memory_set.push(
            MapArea::new(
                VirtAddr(ekernel as usize),
                VirtAddr(MEMORY_END),
                MapType::Identical,
                PTEFlags::R | PTEFlags::W,
            ),
            None,
        );
        // Map MMIO regions
        for &(start, len) in MMIO {
            println!("[MM] Mapping MMIO [{:#x}, {:#x})", start, start + len);
            memory_set.push(
                MapArea::new(
                    VirtAddr(start),
                    VirtAddr(start + len),
                    MapType::Identical,
                    PTEFlags::R | PTEFlags::W,
                ),
                None,
            );
        }
        memory_set
    }

    /// Clone the memory set for fork
    pub fn clone_for_fork(&self) -> Self {
        let mut new_set = Self::new_bare();
        for area in &self.areas {
            let new_area = MapArea::new(
                area.vpn_range.start.addr(),
                area.vpn_range.end.addr(),
                MapType::Framed,
                area.pte_flags,
            );
            new_set.push(new_area, None);
            // Copy data
            for vpn in area.vpn_range.start.0..area.vpn_range.end.0 {
                let vpn = VirtPageNum(vpn);
                if let Some(src_pte) = self.page_table.translate(vpn) {
                    if src_pte.is_valid() {
                        let dst_pte = new_set.page_table.translate(vpn).unwrap();
                        let src = src_pte.ppn().as_bytes_mut();
                        let dst = dst_pte.ppn().as_bytes_mut();
                        dst.copy_from_slice(src);
                    }
                }
            }
        }
        new_set
    }

    pub fn recycle(&mut self) {
        self.areas.clear();
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<super::PageTableEntry> {
        self.page_table.translate(vpn)
    }
}
