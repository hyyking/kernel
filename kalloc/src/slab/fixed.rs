use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;

use libx64::paging::{page::Page, Page4Kb};

use crate::slab::{SlabCheck, SlabSize};

pub struct SlabPage<const N: usize>
where
    SlabCheck<N>: SlabSize,
{
    base: NonNull<u8>,
    mask: u32,
    len: u32,
}

unsafe impl<const N: usize> Send for SlabPage<N> where SlabCheck<N>: SlabSize {}
unsafe impl<const N: usize> Sync for SlabPage<N> where SlabCheck<N>: SlabSize {}

impl<const N: usize> SlabPage<N>
where
    SlabCheck<N>: SlabSize,
{
    const SLOT_BYTES: usize = (N as usize) / 8;

    pub const fn from_page(page: Page<Page4Kb>) -> Self {
        Self {
            base: unsafe { NonNull::new_unchecked(page.ptr().as_u64() as *mut u8) },
            mask: 0,
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn capacity(&self) -> usize {
        Page4Kb as usize / N
    }

    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() > Self::SLOT_BYTES {
            return Err(AllocError);
        }

        let result = if self.len() % 2 == 0 {
            let mut range = 0..(self.capacity() / 2 + 1);
            range.find_map(|i| self.try_alloc_at(i))
        } else {
            let range = (self.capacity() / 2 + 1)..self.capacity();
            range.rev().find_map(|i| self.try_alloc_at(i))
        };

        result.ok_or(AllocError)
    }

    fn try_alloc_at(&mut self, at: usize) -> Option<NonNull<[u8]>> {
        let mask_entry = 1 << at;
        if mask_entry & self.mask == 0 {
            self.mask |= mask_entry;
            self.len += 1;
            let s = unsafe {
                core::slice::from_raw_parts_mut(
                    self.base.as_ptr().add(at * Self::SLOT_BYTES),
                    Self::SLOT_BYTES,
                )
            };
            Some(NonNull::from(s))
        } else {
            None
        }
    }

    fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        assert!(layout.size() <= Self::SLOT_BYTES, "{:?}={:?}", ptr, layout);

        let ptr = ptr.as_ptr() as u64;
        let this = self.base.as_ptr() as u64;

        let offset = (ptr - this) as usize / Self::SLOT_BYTES;
        let mask = 1 << offset;

        if mask & self.mask != 0 {
            self.len -= 1;
            self.mask ^= mask;
        }
    }
}

impl<const N: usize> core::fmt::Debug for SlabPage<N>
where
    SlabCheck<N>: SlabSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SlabPage")
            .field("base", &format_args!("{:#x}", self.base.as_ptr() as u64))
            .field("mask", &format_args!("{:#034b}", self.mask))
            .field("len", &self.len)
            .field("cap", &self.capacity())
            .finish()
    }
}

unsafe impl<const N: usize> Allocator for SlabPage<N>
where
    SlabCheck<N>: SlabSize,
{
    fn allocate(
        &self,
        layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        assert!(layout.size() <= SlabPage::<N>::SLOT_BYTES);

        // FIXME: this is wrong on so many levels
        #[allow(unsafe_op_in_unsafe_fn, unused_unsafe)]
        let this = unsafe { &mut *(self as *const _ as usize as *mut Self) };
        libx64::without_interrupts(|| SlabPage::<N>::allocate(this, layout))
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        assert!(layout.size() <= SlabPage::<N>::SLOT_BYTES);

        // FIXME: this is wrong on so many levels
        #[allow(unsafe_op_in_unsafe_fn, unused_unsafe)]
        let this = unsafe { &mut *(self as *const _ as usize as *mut Self) };
        libx64::without_interrupts(|| SlabPage::<N>::deallocate(this, ptr, layout))
    }
}
