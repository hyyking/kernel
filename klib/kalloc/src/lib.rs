#![no_std]
#![feature(
    allocator_api,
    slice_ptr_get,
    slice_ptr_len,
    core_intrinsics,
    const_assume,
    array_chunks,
    step_trait
)]
#![allow(unsafe_op_in_unsafe_fn, unused_unsafe)]
#![allow(clippy::module_name_repetitions)]

#[cfg(test)]
#[macro_use]
extern crate std;

extern crate alloc;

pub mod buddy;
pub mod kalloc;
pub mod shared;
pub mod slab;

use core::ptr::NonNull;

use libx64::{
    address::VirtualAddr,
    paging::{page::PageRange, Page4Kb},
};

use bitflags::bitflags;

bitflags! {
    pub struct AllocatorBinFlags: u64 {
        const USED = 1;

        const USR_BIT1 = 1 << 60;
        const USR_BIT2 = 1 << 61;
        const USR_BIT3 = 1 << 62;
        const USR_BIT4 = 1 << 63;
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct AllocatorBin {
    pub flags: AllocatorBinFlags,
    pub start: VirtualAddr,
    pub end: VirtualAddr,
    pub data: usize,
}

impl AllocatorBin {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            flags: AllocatorBinFlags::empty(),
            start: VirtualAddr::new(0),
            end: VirtualAddr::new(0),
            data: 0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn with_flags(flags: AllocatorBinFlags) -> Self {
        Self {
            flags,
            start: VirtualAddr::new(0),
            end: VirtualAddr::new(0),
            data: 0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn range(&self) -> PageRange<Page4Kb> {
        PageRange::new_addr(self.start, self.end)
    }

    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.range().len()
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// # Safety
    /// The data pointer must be a pointer to a T
    #[must_use]
    pub unsafe fn cast_data_ptr<T>(&self) -> Option<NonNull<T>> {
        NonNull::new(self.data as *mut T)
    }

    #[must_use]
    pub fn data_ref(&self) -> &usize {
        &self.data
    }

    #[must_use]
    pub fn data_ref_mut(&mut self) -> &mut usize {
        &mut self.data
    }
}

impl core::fmt::Debug for AllocatorBin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AllocatorBin")
            .field("flags", &self.flags)
            .field("range", &self.range())
            .field("data", &self.data)
            .finish()
    }
}
