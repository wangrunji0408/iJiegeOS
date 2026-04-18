use core::fmt;

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SIZE_BITS: usize = 12;

/// Physical address
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct PhysAddr(pub usize);

/// Virtual address
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct VirtAddr(pub usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct PhysPageNum(pub usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct VirtPageNum(pub usize);

impl PhysAddr {
    pub fn floor(self) -> PhysPageNum { PhysPageNum(self.0 / PAGE_SIZE) }
    pub fn ceil(self) -> PhysPageNum { PhysPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE) }
    pub fn page_offset(self) -> usize { self.0 & (PAGE_SIZE - 1) }
    pub fn as_usize(self) -> usize { self.0 }
}

impl VirtAddr {
    pub fn floor(self) -> VirtPageNum { VirtPageNum(self.0 / PAGE_SIZE) }
    pub fn ceil(self) -> VirtPageNum { VirtPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE) }
    pub fn page_offset(self) -> usize { self.0 & (PAGE_SIZE - 1) }
    pub fn as_usize(self) -> usize { self.0 }
}

impl PhysPageNum {
    pub fn base(self) -> PhysAddr { PhysAddr(self.0 * PAGE_SIZE) }
    pub fn get_bytes_array(self) -> &'static mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.base().as_usize() as *mut u8, PAGE_SIZE) }
    }
    pub fn get_pte_array(self) -> &'static mut [super::page_table::PageTableEntry] {
        unsafe { core::slice::from_raw_parts_mut(self.base().as_usize() as *mut _, 512) }
    }
}

impl VirtPageNum {
    pub fn base(self) -> VirtAddr { VirtAddr(self.0 * PAGE_SIZE) }
    /// Get the three 9-bit indices for Sv39
    pub fn indices(self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut out = [0; 3];
        for i in (0..3).rev() {
            out[i] = vpn & 0x1ff;
            vpn >>= 9;
        }
        out
    }
}

impl fmt::Display for PhysAddr { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "PA({:#x})", self.0) } }
impl fmt::Display for VirtAddr { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "VA({:#x})", self.0) } }
