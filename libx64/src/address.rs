use core::ptr::NonNull;

use crate::paging::{PageTableIndex, PageTableLevel};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct VirtualAddr(u64);

impl VirtualAddr {
    pub const fn new(addr: u64) -> Self {
        match addr >> 47 {
            0 | 0x1FFFF => Self(addr),
            1 => Self(((addr << 16) as i64 >> 16) as u64),
            _ => panic!(),
        }
    }

    pub fn ptr<T>(&self) -> Option<NonNull<T>> {
        NonNull::new(self.0 as *mut T)
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as u64)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn null() -> Self {
        Self(0)
    }

    pub const fn page_table_index(self, level: PageTableLevel) -> PageTableIndex {
        PageTableIndex::new_truncate((self.0 >> 12 >> (level as u64 - 1) * 9) as u16)
    }

    pub const fn page_offset(self) -> u16 {
        (self.0 as u16) % (1 << 12)
    }
}

impl core::fmt::Debug for VirtualAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("VirtualAddr")
            .field(&format_args!("{:#x}", self.0))
            .finish()
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PhysicalAddr(u64);

impl PhysicalAddr {
    pub const fn new(addr: u64) -> Self {
        Self(addr % (1 << 52))
    }

    pub fn ptr<T>(&self) -> Option<NonNull<T>> {
        NonNull::new(self.0 as *mut T)
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as u64)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub const fn null() -> Self {
        Self(0)
    }

    pub const fn align_down(self, align: u64) -> Self {
        Self(align_down(self.0, align))
    }
}

impl core::fmt::Debug for PhysicalAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PhysicalAddr")
            .field(&format_args!("{:#x}", self.0))
            .finish()
    }
}

#[inline]
pub const fn align_down(ptr: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    ptr & !(align - 1)
}

#[inline]
pub const fn align_up(addr: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    let align_mask = align - 1;
    if addr & align_mask != 0 {
        return (addr | align_mask) + 1;
    }
    addr
}
