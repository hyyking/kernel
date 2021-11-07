use core::ptr::NonNull;

use crate::paging::table::{PageLevel, PageTableIndex};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct VirtualAddr(u64);

impl VirtualAddr {
    /// # Panics
    ///
    /// Panic if the address is not canonical
    #[inline]
    #[must_use]
    pub const fn new(addr: u64) -> Self {
        match addr >> 47 {
            0 | 0x1FFFF => Self(addr),
            1 => Self(((addr << 16) as i64 >> 16) as u64),
            _ => panic!(),
        }
    }

    #[inline]
    #[must_use]
    pub fn ptr<T>(&self) -> Option<NonNull<T>> {
        NonNull::new(self.0 as *mut T)
    }

    #[inline]
    #[must_use]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as u64)
    }

    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    #[inline]
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline]
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn page_table_index<T: PageLevel>(self, _level: T) -> PageTableIndex<T> {
        PageTableIndex::new_truncate((self.0 >> 12 >> ((T::VALUE - 1) * 9)) as u16)
    }

    #[inline]
    #[must_use]
    pub const fn page_offset(self) -> u16 {
        (self.0 as u16) % (1 << 12)
    }

    #[inline]
    #[must_use]
    pub const fn align_down(self, align: u64) -> Self {
        Self(align_down(self.0, align))
    }

    #[inline]
    #[must_use]
    pub const fn align_up(self, align: u64) -> Self {
        Self(align_up(self.0, align))
    }
}

impl core::fmt::Debug for VirtualAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("VirtualAddr")
            .field(&format_args!("{:#x}", self.0))
            .finish()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct PhysicalAddr(u64);

impl PhysicalAddr {
    #[inline]
    #[must_use]
    pub const fn new(addr: u64) -> Self {
        Self(addr % (1 << 52))
    }

    #[inline]
    #[must_use]
    pub fn ptr<T>(&self) -> Option<NonNull<T>> {
        NonNull::new(self.0 as *mut T)
    }

    #[inline]
    #[must_use]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as u64)
    }

    #[inline]
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    #[inline]
    #[must_use]
    pub const fn align_down(self, align: u64) -> Self {
        Self(align_down(self.0, align))
    }

    #[inline]
    #[must_use]
    pub const fn align_up(self, align: u64) -> Self {
        Self(align_up(self.0, align))
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
const fn align_down(ptr: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    ptr & !(align - 1)
}

#[inline]
const fn align_up(addr: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    let align_mask = align - 1;
    if addr & align_mask != 0 {
        return (addr | align_mask) + 1;
    }
    addr
}

impl core::ops::Add<u64> for VirtualAddr {
    type Output = VirtualAddr;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl core::ops::Sub<u64> for VirtualAddr {
    type Output = VirtualAddr;

    fn sub(self, rhs: u64) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl core::ops::Add<u64> for PhysicalAddr {
    type Output = PhysicalAddr;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl core::ops::Sub<u64> for PhysicalAddr {
    type Output = PhysicalAddr;

    fn sub(self, rhs: u64) -> Self::Output {
        Self(self.0 - rhs)
    }
}
