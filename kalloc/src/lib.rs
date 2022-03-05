#![no_std]
#![feature(allocator_api, slice_ptr_get, slice_ptr_len)]

extern crate alloc;

#[cfg(test)]
#[macro_use]
extern crate std;

pub mod btree;
pub mod buddy;
pub mod shared;
pub mod slab;

use core::ptr::NonNull;

use libx64::{
    address::VirtualAddr,
    paging::{page::PageRange, Page4Kb},
};

use bitflags::bitflags;

bitflags! {
    struct AllocatorBinFlags: u64 {
        const USED = 1;

        const USR_BIT1 = 1 << 60;
        const USR_BIT2 = 1 << 61;
        const USR_BIT3 = 1 << 62;
        const USR_BIT4 = 1 << 63;
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct AllocatorBin {
    flags: AllocatorBinFlags,
    start: VirtualAddr,
    end: VirtualAddr,
    data: usize,
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

impl AllocatorBin {
    pub unsafe fn cast_data_ptr<T>(&self) -> Option<NonNull<T>> {
        NonNull::new(self.data as *mut T)
    }
}

impl AllocatorBin {
    pub const fn new() -> Self {
        Self {
            flags: AllocatorBinFlags::empty(),
            start: VirtualAddr::new(0),
            end: VirtualAddr::new(0),
            data: 0,
        }
    }

    pub const fn range(&self) -> PageRange<Page4Kb> {
        PageRange::new_addr(self.start, self.end)
    }

    pub const fn len(&self) -> usize {
        self.range().len()
    }
}
