use crate::config::{PAGE_SIZE, PAGE_SIZE_BITS};

/// Physical address
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct PhysAddr(pub usize);

/// Virtual address
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct VirtAddr(pub usize);

/// Physical page number
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct PhysPageNum(pub usize);

/// Virtual page number
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct VirtPageNum(pub usize);

impl PhysAddr {
    pub fn page_number(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }
    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }
    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }
}

impl VirtAddr {
    pub fn page_number(&self) -> VirtPageNum {
        VirtPageNum(self.0 / PAGE_SIZE)
    }
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }
    pub fn ceil(&self) -> VirtPageNum {
        VirtPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }
    pub fn floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 / PAGE_SIZE)
    }
}

impl PhysPageNum {
    pub fn addr(&self) -> PhysAddr {
        PhysAddr(self.0 << PAGE_SIZE_BITS)
    }
    /// Get a mutable reference to the page as a byte slice
    pub fn as_bytes_mut(&self) -> &'static mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self.addr().0 as *mut u8, PAGE_SIZE)
        }
    }
    /// Get the page table entries array on this physical page
    pub fn as_pte_array(&self) -> &'static mut [PageTableEntry] {
        unsafe {
            core::slice::from_raw_parts_mut(self.addr().0 as *mut PageTableEntry, 512)
        }
    }
}

impl VirtPageNum {
    pub fn addr(&self) -> VirtAddr {
        VirtAddr(self.0 << PAGE_SIZE_BITS)
    }
    /// Get the three-level page table indices for Sv39
    pub fn indices(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0usize; 3];
        for i in (0..3).rev() {
            idx[i] = vpn & 511;
            vpn >>= 9;
        }
        idx
    }
}

impl From<usize> for PhysAddr {
    fn from(v: usize) -> Self { PhysAddr(v) }
}
impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self { VirtAddr(v) }
}
impl From<usize> for PhysPageNum {
    fn from(v: usize) -> Self { PhysPageNum(v) }
}
impl From<usize> for VirtPageNum {
    fn from(v: usize) -> Self { VirtPageNum(v) }
}
impl From<PhysAddr> for usize {
    fn from(v: PhysAddr) -> Self { v.0 }
}
impl From<VirtAddr> for usize {
    fn from(v: VirtAddr) -> Self { v.0 }
}
impl From<PhysPageNum> for usize {
    fn from(v: PhysPageNum) -> Self { v.0 }
}
impl From<VirtPageNum> for usize {
    fn from(v: VirtPageNum) -> Self { v.0 }
}

use super::PageTableEntry;
