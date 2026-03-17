use super::address::*;
use super::frame::{frame_alloc, FrameTracker};
use super::page_table::{PageTable, PTEFlags, PageTableEntry};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use bitflags::bitflags;
use spin::Mutex;
use lazy_static::lazy_static;

bitflags! {
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

impl From<MapPermission> for PTEFlags {
    fn from(perm: MapPermission) -> Self {
        PTEFlags::from_bits_truncate(perm.bits())
    }
}

/// 映射类型
#[derive(Clone, PartialEq, Debug)]
pub enum MapType {
    /// 恒等映射（物理地址 = 虚拟地址）
    Identical,
    /// 分配新物理页
    Framed,
    /// 懒分配（访问时才分配物理页）
    Lazy,
}

/// 内存区域（VMA）
pub struct MapArea {
    pub vpn_range: VPNRange,
    pub data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    pub map_type: MapType,
    pub map_perm: MapPermission,
    pub name: &'static str,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
        name: &'static str,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
            name,
        }
    }

    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().expect("OOM: map area");
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
            MapType::Lazy => {
                // 懒分配不立即分配物理页
                return;
            }
        }
        let pte_flags = PTEFlags::from(self.map_perm);
        page_table.map(vpn, ppn, pte_flags);
    }

    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }

    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range.clone().into_iter() {
            self.map_one(page_table, vpn);
        }
    }

    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range.clone().into_iter() {
            self.unmap_one(page_table, vpn);
        }
    }

    /// 将数据复制到此区域
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8], offset: usize) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();

        // 跳过 offset 对应的页
        let skip_pages = offset / PAGE_SIZE;
        for _ in 0..skip_pages {
            current_vpn.step();
            start += PAGE_SIZE;
        }

        let page_offset = offset % PAGE_SIZE;

        loop {
            let src = &data[start..len.min(start + PAGE_SIZE - page_offset + if start == 0 { page_offset } else { 0 })];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array();

            let dst_offset = if start == 0 { page_offset } else { 0 };
            dst[dst_offset..dst_offset + src.len()].copy_from_slice(src);
            start += src.len();
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
}

impl Clone for MapArea {
    fn clone(&self) -> Self {
        Self {
            vpn_range: VPNRange::new(
                self.vpn_range.get_start(),
                self.vpn_range.get_end()
            ),
            data_frames: BTreeMap::new(), // 子进程需要 fork 时复制数据
            map_type: self.map_type.clone(),
            map_perm: self.map_perm,
            name: self.name,
        }
    }
}

/// 进程地址空间
pub struct MemorySet {
    page_table: PageTable,
    pub areas: Vec<MapArea>,
    /// mmap 区域（通过 mmap 系统调用创建）
    pub mmap_areas: Vec<MmapArea>,
}

/// mmap 区域描述
pub struct MmapArea {
    pub start: usize,
    pub end: usize,
    pub prot: usize,
    pub flags: usize,
    pub data_frames: BTreeMap<VirtPageNum, FrameTracker>,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
            mmap_areas: Vec::new(),
        }
    }

    pub fn token(&self) -> usize {
        self.page_table.token()
    }

    pub fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, data, 0);
        }
        self.areas.push(map_area);
    }

    pub fn push_with_offset(&mut self, mut map_area: MapArea, data: &[u8], offset: usize) {
        map_area.map(&mut self.page_table);
        map_area.copy_data(&mut self.page_table, data, offset);
        self.areas.push(map_area);
    }

    /// 插入帧映射（不加入areas管理，用于临时映射）
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission, "anon"),
            None,
        );
    }

    /// mmap 分配
    pub fn mmap(&mut self, start: usize, len: usize, prot: usize) -> usize {
        // 找一个合适的虚拟地址
        let start = if start == 0 {
            self.find_free_area(len)
        } else {
            start
        };
        let end = start + len;

        // 创建懒分配区域
        let mut area = MapArea::new(
            VirtAddr::from(start),
            VirtAddr::from(end),
            MapType::Lazy,
            MapPermission::from_bits_truncate(prot as u8),
            "mmap",
        );

        // 对于 Lazy 映射，我们需要在页表中记录，但不分配物理页
        // 实际上，我们把mmap区域单独追踪
        self.mmap_areas.push(MmapArea {
            start,
            end,
            prot,
            flags: 0,
            data_frames: BTreeMap::new(),
        });

        start
    }

    /// 找一个空闲虚拟地址区域
    fn find_free_area(&self, len: usize) -> usize {
        // 简单策略：从 0x40000000 开始往上找
        let mut candidate = 0x40000000usize;
        'outer: loop {
            let end = candidate + len;
            // 检查是否与现有映射冲突
            for area in &self.areas {
                let area_start: usize = VirtAddr::from(area.vpn_range.get_start()).into();
                let area_end: usize = VirtAddr::from(area.vpn_range.get_end()).into();
                if candidate < area_end && end > area_start {
                    candidate = (area_end + 4095) & !4095;
                    continue 'outer;
                }
            }
            for mmap in &self.mmap_areas {
                if candidate < mmap.end && end > mmap.start {
                    candidate = (mmap.end + 4095) & !4095;
                    continue 'outer;
                }
            }
            break;
        }
        candidate
    }

    pub fn munmap(&mut self, start: usize, len: usize) {
        let end = start + len;
        self.mmap_areas.retain(|area| {
            !(area.start >= start && area.end <= end)
        });
    }

    pub fn find_mmap_area(&self, addr: usize) -> Option<usize> {
        for (i, area) in self.mmap_areas.iter().enumerate() {
            if addr >= area.start && addr < area.end {
                return Some(i);
            }
        }
        None
    }

    pub fn handle_cow_fault(&mut self, addr: usize) -> bool {
        let vpn = VirtAddr::from(addr).floor();
        for area in &mut self.mmap_areas {
            if addr >= area.start && addr < area.end {
                // 分配物理页
                if let Some(frame) = frame_alloc() {
                    let ppn = frame.ppn;
                    let prot = area.prot;
                    let mut flags = PTEFlags::V | PTEFlags::A | PTEFlags::D | PTEFlags::U;
                    if prot & 1 != 0 { flags |= PTEFlags::R; }
                    if prot & 2 != 0 { flags |= PTEFlags::W; }
                    if prot & 4 != 0 { flags |= PTEFlags::X; }
                    self.page_table.map(vpn, ppn, flags);
                    area.data_frames.insert(vpn, frame);
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// 为用户进程创建地址空间
    pub fn new_user(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // 映射 Trampoline
        memory_set.map_trampoline();
        // 解析 ELF
        crate::loader::load_elf(&mut memory_set, elf_data)
    }

    /// 映射 Trampoline（内核陷阱代码）到用户空间最高地址
    fn map_trampoline(&mut self) {
        extern "C" {
            fn strampoline();
        }
        self.page_table.map(
            VirtAddr::from(super::super::mm::TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }

    /// 内核地址空间
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();

        // 映射 Trampoline
        memory_set.map_trampoline();

        // 内核各段的恒等映射
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

        // .text 段：可读可执行
        memory_set.push(MapArea::new(
            (stext as usize).into(),
            (etext as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::X,
            ".text",
        ), None);

        // .rodata 段：只读
        memory_set.push(MapArea::new(
            (srodata as usize).into(),
            (erodata as usize).into(),
            MapType::Identical,
            MapPermission::R,
            ".rodata",
        ), None);

        // .data 段：可读写
        memory_set.push(MapArea::new(
            (sdata as usize).into(),
            (edata as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            ".data",
        ), None);

        // .bss 段：可读写
        memory_set.push(MapArea::new(
            (sbss as usize).into(),
            (ebss as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            ".bss",
        ), None);

        // 物理内存（恒等映射，用于内核访问任意物理地址）
        memory_set.push(MapArea::new(
            (ekernel as usize).into(),
            super::MEMORY_END.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "phys_mem",
        ), None);

        // MMIO 区域（设备寄存器）
        // UART: 0x10000000
        memory_set.push(MapArea::new(
            0x10000000usize.into(),
            0x10001000usize.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "uart",
        ), None);

        // PLIC: 0xc000000
        memory_set.push(MapArea::new(
            0xc000000usize.into(),
            0xc400000usize.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "plic",
        ), None);

        // VirtIO: 0x10001000 - 0x10008000
        memory_set.push(MapArea::new(
            0x10001000usize.into(),
            0x10009000usize.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "virtio",
        ), None);

        log::info!("Kernel address space created");
        memory_set
    }

    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            riscv::register::satp::write(satp);
            core::arch::asm!("sfence.vma");
        }
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }

    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.page_table.translate_va(va)
    }

    /// fork: 复制地址空间
    pub fn fork_from(user_space: &MemorySet) -> MemorySet {
        let mut memory_set = Self::new_bare();
        // 映射 Trampoline
        memory_set.map_trampoline();

        // 复制所有区域
        for area in user_space.areas.iter() {
            let new_area = area.clone();
            memory_set.push(new_area, None);
            // 复制数据
            for vpn in area.vpn_range.clone().into_iter() {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                let dst_ppn = memory_set.translate(vpn).unwrap().ppn();
                dst_ppn.get_bytes_array().copy_from_slice(src_ppn.get_bytes_array());
            }
        }

        // 复制 mmap 区域
        for mmap in &user_space.mmap_areas {
            let mut new_mmap = MmapArea {
                start: mmap.start,
                end: mmap.end,
                prot: mmap.prot,
                flags: mmap.flags,
                data_frames: BTreeMap::new(),
            };
            // 复制已分配的 mmap 物理页
            for (vpn, src_frame) in &mmap.data_frames {
                let dst_frame = frame_alloc().expect("OOM: mmap fork");
                dst_frame.ppn.get_bytes_array().copy_from_slice(src_frame.ppn.get_bytes_array());
                let prot = mmap.prot;
                let mut flags = PTEFlags::V | PTEFlags::A | PTEFlags::D | PTEFlags::U;
                if prot & 1 != 0 { flags |= PTEFlags::R; }
                if prot & 2 != 0 { flags |= PTEFlags::W; }
                if prot & 4 != 0 { flags |= PTEFlags::X; }
                memory_set.page_table.map(*vpn, dst_frame.ppn, flags);
                new_mmap.data_frames.insert(*vpn, dst_frame);
            }
            memory_set.mmap_areas.push(new_mmap);
        }

        memory_set
    }

    /// 修改虚拟内存区域权限
    pub fn mprotect(&mut self, addr: usize, len: usize, prot: usize) {
        let start_vpn = VirtAddr::from(addr).floor();
        let end_vpn = VirtAddr::from(addr + len).ceil();
        let mut flags = PTEFlags::V | PTEFlags::A | PTEFlags::D | PTEFlags::U;
        if prot & 1 != 0 { flags |= PTEFlags::R; }
        if prot & 2 != 0 { flags |= PTEFlags::W; }
        if prot & 4 != 0 { flags |= PTEFlags::X; }
        for vpn in VPNRange::new(start_vpn, end_vpn).into_iter() {
            self.page_table.set_flags(vpn, flags);
        }
    }
}

lazy_static! {
    pub static ref KERNEL_SPACE: Mutex<MemorySet> = Mutex::new(MemorySet::new_kernel());
}

pub fn init_kernel_space() {
    KERNEL_SPACE.lock().activate();
}
