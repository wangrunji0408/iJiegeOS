/// RISC-V Sv39 虚拟地址和物理地址类型

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SIZE_BITS: usize = 12;

/// Sv39: 物理地址 56 位
pub const PA_WIDTH: usize = 56;
/// Sv39: 虚拟地址 39 位
pub const VA_WIDTH: usize = 39;

pub const PPN_WIDTH: usize = PA_WIDTH - PAGE_SIZE_BITS; // 44位
pub const VPN_WIDTH: usize = VA_WIDTH - PAGE_SIZE_BITS; // 27位

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PhysAddr(pub usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct VirtAddr(pub usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PhysPageNum(pub usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct VirtPageNum(pub usize);

impl PhysAddr {
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }

    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }

    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }

    pub fn get_mut<T>(&self) -> &'static mut T {
        unsafe { &mut *(self.0 as *mut T) }
    }

    pub fn get_ref<T>(&self) -> &'static T {
        unsafe { &*(self.0 as *const T) }
    }
}

impl From<PhysAddr> for usize {
    fn from(v: PhysAddr) -> Self { v.0 }
}

impl From<usize> for PhysAddr {
    fn from(v: usize) -> Self { Self(v & ((1 << PA_WIDTH) - 1)) }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(v: PhysPageNum) -> Self { Self(v.0 << PAGE_SIZE_BITS) }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(v: PhysAddr) -> Self {
        assert_eq!(v.page_offset(), 0, "PhysAddr {:?} not page-aligned", v);
        v.floor()
    }
}

impl From<usize> for PhysPageNum {
    fn from(v: usize) -> Self { Self(v & ((1 << PPN_WIDTH) - 1)) }
}

impl From<PhysPageNum> for usize {
    fn from(v: PhysPageNum) -> Self { v.0 }
}

impl VirtAddr {
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }

    pub fn floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 / PAGE_SIZE)
    }

    pub fn ceil(&self) -> VirtPageNum {
        VirtPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }
}

impl From<VirtAddr> for usize {
    fn from(v: VirtAddr) -> Self { v.0 }
}

impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self { Self(v) }
}

impl From<VirtPageNum> for VirtAddr {
    fn from(v: VirtPageNum) -> Self { Self(v.0 << PAGE_SIZE_BITS) }
}

impl From<VirtAddr> for VirtPageNum {
    fn from(v: VirtAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        v.floor()
    }
}

impl From<usize> for VirtPageNum {
    fn from(v: usize) -> Self { Self(v) }
}

impl From<VirtPageNum> for usize {
    fn from(v: VirtPageNum) -> Self { v.0 }
}

impl PhysPageNum {
    pub fn get_bytes_array(&self) -> &'static mut [u8] {
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut u8, PAGE_SIZE) }
    }

    pub fn get_pte_array(&self) -> &'static mut [crate::mm::PageTableEntry] {
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut crate::mm::PageTableEntry, 512) }
    }

    pub fn get_mut<T>(&self) -> &'static mut T {
        let pa: PhysAddr = (*self).into();
        pa.get_mut()
    }
}

impl VirtPageNum {
    /// 获取 VPN 的三级索引
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0usize; 3];
        for i in (0..3).rev() {
            idx[i] = vpn & 511;
            vpn >>= 9;
        }
        idx
    }
}

pub trait StepByOne: Sized {
    fn step(&mut self);
}

impl StepByOne for VirtPageNum {
    fn step(&mut self) {
        self.0 += 1;
    }
}

impl StepByOne for PhysPageNum {
    fn step(&mut self) {
        self.0 += 1;
    }
}

#[derive(Clone)]
pub struct SimpleRange<T> where T: StepByOne + Copy + PartialEq + PartialOrd + core::fmt::Debug {
    l: T,
    r: T,
}

impl<T> SimpleRange<T> where T: StepByOne + Copy + PartialEq + PartialOrd + core::fmt::Debug {
    pub fn new(start: T, end: T) -> Self {
        assert!(start <= end, "start {:?} > end {:?}", start, end);
        Self { l: start, r: end }
    }

    pub fn get_start(&self) -> T { self.l }
    pub fn get_end(&self) -> T { self.r }
}

impl<T> IntoIterator for SimpleRange<T>
where T: StepByOne + Copy + PartialEq + PartialOrd + core::fmt::Debug {
    type Item = T;
    type IntoIter = SimpleRangeIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        SimpleRangeIterator::new(self.l, self.r)
    }
}

pub struct SimpleRangeIterator<T> where T: StepByOne + Copy + PartialEq + PartialOrd {
    current: T,
    end: T,
}

impl<T> SimpleRangeIterator<T> where T: StepByOne + Copy + PartialEq + PartialOrd {
    pub fn new(l: T, r: T) -> Self {
        Self { current: l, end: r }
    }
}

impl<T> Iterator for SimpleRangeIterator<T>
where T: StepByOne + Copy + PartialEq + PartialOrd {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            None
        } else {
            let t = self.current;
            self.current.step();
            Some(t)
        }
    }
}

pub type VPNRange = SimpleRange<VirtPageNum>;
