use super::address::*;
use super::frame::{frame_alloc, FrameTracker};
use super::page_table::{PageTable, PTEFlags, PageTableEntry};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use bitflags::bitflags;
use spin::Mutex;
use lazy_static::lazy_static;

bitflags! {
    #[derive(Copy, Clone)]
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

#[derive(Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,
    Framed,
    Lazy,
}

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
                let frame = frame_alloc().expect("OOM");
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
            MapType::Lazy => {
                return;
            }
        }
        let pte_flags = PTEFlags::from(self.map_perm) | PTEFlags::V | PTEFlags::A | PTEFlags::D;
        if page_table.translate(vpn).map(|e| e.is_valid()).unwrap_or(false) {
            // 已映射，更新标志
            page_table.set_flags(vpn, pte_flags);
        } else {
            page_table.map(vpn, ppn, pte_flags);
        }
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

    /// 将数据写入区域（从 file_offset 字节偏移处写入 data）
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8], file_offset: usize) {
        let start_vpn = self.vpn_range.get_start();
        let mut vpn = start_vpn;
        let mut written = 0;
        let len = data.len();

        // 计算第一个页的写入偏移
        let first_page_offset = file_offset % PAGE_SIZE;
        let skip_pages = file_offset / PAGE_SIZE;

        // 跳过 skip_pages 个页
        for _ in 0..skip_pages {
            vpn.step();
        }

        while written < len {
            let page_offset = if written == 0 { first_page_offset } else { 0 };
            let available = PAGE_SIZE - page_offset;
            let to_write = (len - written).min(available);

            if let Some(pte) = page_table.translate(vpn) {
                let ppn = pte.ppn();
                let page_bytes = ppn.get_bytes_array();
                page_bytes[page_offset..page_offset + to_write]
                    .copy_from_slice(&data[written..written + to_write]);
            }

            written += to_write;
            vpn.step();
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
            data_frames: BTreeMap::new(),
            map_type: self.map_type.clone(),
            map_perm: self.map_perm,
            name: self.name,
        }
    }
}

pub struct MmapArea {
    pub start: usize,
    pub end: usize,
    pub prot: usize,
    pub flags: usize,
    pub data_frames: BTreeMap<VirtPageNum, FrameTracker>,
}

pub struct MemorySet {
    pub page_table: PageTable,
    pub areas: Vec<MapArea>,
    pub mmap_areas: Vec<MmapArea>,
    pub brk: usize,
    pub brk_start: usize,
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
            mmap_areas: Vec::new(),
            brk: 0,
            brk_start: 0,
        }
    }

    /// 创建用户地址空间（包含内核恒等映射以支持陷阱处理）
    pub fn new_user_bare() -> Self {
        let mut ms = Self::new_bare();
        // 在用户页表中映射内核区域（恒等映射）
        // trap 发生时 CPU 还在用户页表下跳到 stvec，需要能访问内核代码和数据
        // 内核 satp=0 时，所有操作都在用户页表下进行
        extern "C" {
            fn stext();
            fn ekernel();
        }

        // 映射整个内核（text + rodata + data + bss）为 R|W|X
        // 因为 trap_handler 需要在用户页表下读写内核数据
        ms.push(MapArea::new(
            (stext as usize).into(),
            (ekernel as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W | MapPermission::X,
            "kernel_in_user",
        ), None);

        // 剩余物理内存（页帧分配器管理的内存，用于 TrapContext 等分配的帧）
        ms.push(MapArea::new(
            (ekernel as usize).into(),
            super::MEMORY_END.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "phys_mem_in_user",
        ), None);

        // UART 和其他 MMIO
        ms.push(MapArea::new(
            0x10000000usize.into(),
            0x10009000usize.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "mmio_in_user",
        ), None);

        ms
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

    pub fn push_with_offset(&mut self, mut map_area: MapArea, data: &[u8], file_offset: usize) {
        map_area.map(&mut self.page_table);
        map_area.copy_data(&mut self.page_table, data, file_offset);
        self.areas.push(map_area);
    }

    pub fn insert_framed_area(&mut self, start_va: VirtAddr, end_va: VirtAddr, perm: MapPermission) {
        self.push(MapArea::new(start_va, end_va, MapType::Framed, perm, "anon"), None);
    }

    pub fn mmap_fixed(&mut self, start: usize, end: usize, prot: usize) {
        let mut area = MmapArea {
            start, end, prot, flags: 0,
            data_frames: BTreeMap::new(),
        };
        // 立即分配并映射所有页
        let start_vpn = VirtAddr::from(start).floor();
        let end_vpn = VirtAddr::from(end).ceil();
        let mut flags = PTEFlags::V | PTEFlags::A | PTEFlags::D | PTEFlags::U;
        if prot & 1 != 0 { flags |= PTEFlags::R; }
        if prot & 2 != 0 { flags |= PTEFlags::W; }
        if prot & 4 != 0 { flags |= PTEFlags::X; }
        for vpn in VPNRange::new(start_vpn, end_vpn).into_iter() {
            if let Some(frame) = frame_alloc() {
                let ppn = frame.ppn;
                if self.page_table.translate(vpn).map(|e| e.is_valid()).unwrap_or(false) {
                    self.page_table.set_flags(vpn, flags);
                } else {
                    self.page_table.map(vpn, ppn, flags);
                }
                area.data_frames.insert(vpn, frame);
            }
        }
        self.mmap_areas.push(area);
    }

    pub fn mmap(&mut self, hint: usize, len: usize, prot: usize) -> usize {
        let start = if hint == 0 {
            self.find_free_area(len)
        } else {
            // MAP_FIXED: 使用精确地址（页对齐）
            hint & !4095
        };
        let end = (start + len + 4095) & !4095;

        let mut area = MmapArea {
            start, end, prot, flags: 0,
            data_frames: BTreeMap::new(),
        };

        // 立即分配
        let start_vpn = VirtAddr::from(start).floor();
        let end_vpn = VirtAddr::from(end).ceil();
        let mut flags = PTEFlags::V | PTEFlags::A | PTEFlags::D | PTEFlags::U;
        if prot & 1 != 0 { flags |= PTEFlags::R; }
        if prot & 2 != 0 { flags |= PTEFlags::W; }
        if prot & 4 != 0 { flags |= PTEFlags::X; }
        for vpn in VPNRange::new(start_vpn, end_vpn).into_iter() {
            if self.page_table.translate(vpn).map(|e| e.is_valid()).unwrap_or(false) {
                // 已有映射：仅更新权限（MAP_FIXED 覆盖已有映射）
                self.page_table.set_flags(vpn, flags);
            } else {
                // 分配新帧
                if let Some(frame) = frame_alloc() {
                    let ppn = frame.ppn;
                    self.page_table.map(vpn, ppn, flags);
                    area.data_frames.insert(vpn, frame);
                }
            }
        }

        self.mmap_areas.push(area);
        start
    }

    pub fn munmap(&mut self, start: usize, len: usize) {
        let end = start + len;
        let start_vpn = VirtAddr::from(start).floor();
        let end_vpn = VirtAddr::from(end).ceil();

        self.mmap_areas.retain(|area| {
            if area.start >= start && area.end <= end {
                // 解映射此区域
                for vpn in VPNRange::new(
                    VirtAddr::from(area.start).floor(),
                    VirtAddr::from(area.end).ceil()
                ).into_iter() {
                    if self.page_table.translate(vpn).map(|e| e.is_valid()).unwrap_or(false) {
                        // 不能在这里 unmap（借用冲突）
                    }
                }
                false
            } else {
                true
            }
        });

        // 在页表中解映射
        for vpn in VPNRange::new(start_vpn, end_vpn).into_iter() {
            if self.page_table.translate(vpn).map(|e| e.is_valid()).unwrap_or(false) {
                self.page_table.unmap(vpn);
            }
        }
    }

    fn find_free_area(&self, len: usize) -> usize {
        // 从 0x70000000 开始查找（高于 brk 区域，避免冲突）
        let mut candidate = 0x70000000usize;
        loop {
            let end = (candidate + len + 4095) & !4095;
            let mut conflict = false;

            // 检查 brk 区域
            if self.brk_start > 0 {
                let brk_end = (self.brk + 4095) & !4095;
                if candidate < brk_end && end > self.brk_start {
                    candidate = (brk_end + 4095) & !4095;
                    conflict = true;
                }
            }

            if !conflict {
                for area in &self.areas {
                    let area_start: usize = VirtAddr::from(area.vpn_range.get_start()).into();
                    let area_end: usize = VirtAddr::from(area.vpn_range.get_end()).into();
                    if candidate < area_end && end > area_start {
                        candidate = (area_end + 4095) & !4095;
                        conflict = true;
                        break;
                    }
                }
            }
            if !conflict {
                for mmap in &self.mmap_areas {
                    if candidate < mmap.end && end > mmap.start {
                        candidate = (mmap.end + 4095) & !4095;
                        conflict = true;
                        break;
                    }
                }
            }
            if !conflict {
                break;
            }
        }
        candidate
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
                if let Some(frame) = frame_alloc() {
                    let ppn = frame.ppn;
                    let prot = area.prot;
                    let mut flags = PTEFlags::V | PTEFlags::A | PTEFlags::D | PTEFlags::U;
                    if prot & 1 != 0 { flags |= PTEFlags::R; }
                    if prot & 2 != 0 { flags |= PTEFlags::W; }
                    if prot & 4 != 0 { flags |= PTEFlags::X; }
                    if self.page_table.translate(vpn).map(|e| e.is_valid()).unwrap_or(false) {
                        self.page_table.set_flags(vpn, flags);
                    } else {
                        self.page_table.map(vpn, ppn, flags);
                        area.data_frames.insert(vpn, frame);
                    }
                    return true;
                }
                return false;
            }
        }
        false
    }

    pub fn set_brk(&mut self, new_brk: usize) -> usize {
        if new_brk <= self.brk_start {
            return self.brk;
        }
        let old_end_vpn = VirtAddr::from(self.brk).ceil();
        let new_end_vpn = VirtAddr::from(new_brk).ceil();

        if new_end_vpn > old_end_vpn {
            // 需要分配新页
            let flags = PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::U | PTEFlags::A | PTEFlags::D;
            for vpn in VPNRange::new(old_end_vpn, new_end_vpn).into_iter() {
                if let Some(frame) = frame_alloc() {
                    let ppn = frame.ppn;
                    self.page_table.map(vpn, ppn, flags);
                    // 需要记录这个 frame... 简化处理：用 area 管理
                    // 实际上需要一个 heap area
                    // 把 frame 放入一个专门的 heap_frames（这里简化）
                }
            }
        }
        self.brk = new_brk;
        self.brk
    }

    /// 从 ELF 创建用户地址空间
    pub fn new_user(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_user_bare();  // 包含内核映射
        crate::loader::load_elf(&mut memory_set, elf_data)
    }

    /// 内核地址空间
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
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

        // .text 段
        memory_set.push(MapArea::new(
            (stext as usize).into(),
            (etext as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::X,
            ".text",
        ), None);

        // .rodata
        memory_set.push(MapArea::new(
            (srodata as usize).into(),
            (erodata as usize).into(),
            MapType::Identical,
            MapPermission::R,
            ".rodata",
        ), None);

        // .data + .bss.stack（包含启动栈）
        memory_set.push(MapArea::new(
            (sdata as usize).into(),
            (sbss as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            ".data+stack",
        ), None);

        // .bss（含内核堆 HEAP_SPACE）
        memory_set.push(MapArea::new(
            (sbss as usize).into(),
            (ebss as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            ".bss",
        ), None);

        // 物理内存（内核堆 + 页帧分配器）
        memory_set.push(MapArea::new(
            (ekernel as usize).into(),
            super::MEMORY_END.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "phys_mem",
        ), None);

        // MMIO 区域
        // UART 16550A: 0x10000000
        memory_set.push(MapArea::new(
            0x10000000usize.into(),
            0x10001000usize.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "uart0",
        ), None);

        // PLIC: 0x0c000000
        memory_set.push(MapArea::new(
            0x0c000000usize.into(),
            0x10000000usize.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "plic",
        ), None);

        // VirtIO 设备: 0x10001000 - 0x10009000
        memory_set.push(MapArea::new(
            0x10001000usize.into(),
            0x10009000usize.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
            "virtio",
        ), None);

        log::info!("Kernel memory set created");
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

    /// Fork 时复制地址空间
    pub fn fork_from(user_space: &MemorySet) -> MemorySet {
        let mut memory_set = Self::new_bare();

        for area in user_space.areas.iter() {
            let mut new_area = area.clone();
            // 重新映射
            for vpn in area.vpn_range.clone().into_iter() {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                if let Some(frame) = frame_alloc() {
                    frame.ppn.get_bytes_array().copy_from_slice(src_ppn.get_bytes_array());
                    let flags = user_space.page_table.translate(vpn).unwrap().flags();
                    memory_set.page_table.map(vpn, frame.ppn, flags);
                    new_area.data_frames.insert(vpn, frame);
                }
            }
            memory_set.areas.push(new_area);
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
            for (vpn, src_frame) in &mmap.data_frames {
                if let Some(dst_frame) = frame_alloc() {
                    dst_frame.ppn.get_bytes_array().copy_from_slice(src_frame.ppn.get_bytes_array());
                    let flags = user_space.page_table.translate(*vpn).unwrap().flags();
                    memory_set.page_table.map(*vpn, dst_frame.ppn, flags);
                    new_mmap.data_frames.insert(*vpn, dst_frame);
                }
            }
            memory_set.mmap_areas.push(new_mmap);
        }

        memory_set.brk = user_space.brk;
        memory_set.brk_start = user_space.brk_start;
        memory_set
    }
}

lazy_static! {
    pub static ref KERNEL_SPACE: Mutex<MemorySet> = Mutex::new(MemorySet::new_kernel());
}

pub fn init_kernel_space() {
    // 仅初始化内核空间，不激活页表
    // 内核使用恒等映射，激活页表在 trap 初始化之后进行
    let _ = KERNEL_SPACE.lock();
    log::info!("Kernel space initialized (not activated yet)");
}

pub fn activate_kernel_space() {
    // 激活内核页表（在 trap 初始化之后调用）
    KERNEL_SPACE.lock().activate();
    log::info!("Kernel page table activated");
}
