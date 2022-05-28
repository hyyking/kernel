use alloc::alloc::{AllocError, Layout};
use core::ptr::NonNull;

use crate::kalloc::AllocatorMutImpl;

use libx64::paging::{page::Page, Page4Kb};

pub struct SlabPage {
    base: NonNull<u8>,
    mask: u32,
    len: u32,
}

const N: usize = 4096;

unsafe impl Send for SlabPage {}
unsafe impl Sync for SlabPage {}

impl SlabPage {
    const SLOT_BYTES: usize = (N as usize) / 8;

    #[inline]
    #[must_use]
    pub const fn from_page(page: Page<Page4Kb>) -> Self {
        Self {
            base: unsafe { NonNull::new_unchecked(page.ptr().as_u64() as *mut u8) },
            mask: 0,
            len: 0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    #[must_use]
    pub const fn capacity() -> usize {
        Page4Kb as usize / N
    }

    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() > Self::SLOT_BYTES {
            return Err(AllocError);
        }

        let result = if self.len() % 2 == 0 {
            let mut range = 0..=(Self::capacity() / 2);
            range.find_map(|i| self.try_alloc_at(i))
        } else {
            let range = (Self::capacity() / 2 + 1)..Self::capacity();
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

        #[allow(clippy::cast_possible_truncation)]
        let offset = (ptr - this) as usize / Self::SLOT_BYTES;
        let mask = 1 << offset;

        if mask & self.mask != 0 {
            self.len -= 1;
            self.mask ^= mask;
        }
    }
}

impl core::fmt::Debug for SlabPage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SlabPage")
            .field("base", &format_args!("{:#x}", self.base.as_ptr() as u64))
            .field("mask", &format_args!("{:#034b}", self.mask))
            .field("len", &self.len)
            .field("cap", &Self::capacity())
            .finish()
    }
}

unsafe impl AllocatorMutImpl for SlabPage {
    fn allocate_mut(
        &mut self,
        layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        assert!(layout.size() <= SlabPage::SLOT_BYTES);

        // FIXME: this is wrong on so many levels
        libx64::without_interrupts(|| SlabPage::allocate(self, layout))
    }

    unsafe fn deallocate_mut(&mut self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        assert!(layout.size() <= SlabPage::SLOT_BYTES);

        // FIXME: this is wrong on so many levels
        #[allow(unsafe_op_in_unsafe_fn, unused_unsafe)]
        libx64::without_interrupts(|| SlabPage::deallocate(self, ptr, layout));
    }
}
