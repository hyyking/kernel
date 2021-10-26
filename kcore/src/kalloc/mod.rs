use alloc::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;

use libx64::paging::{page::Page, Page4Kb};

pub trait SlabSize {}

pub struct SlabCheck<const N: u64>;
impl SlabSize for SlabCheck<128> {}
impl SlabSize for SlabCheck<256> {}
impl SlabSize for SlabCheck<512> {}
impl SlabSize for SlabCheck<1024> {}

pub struct SlabPage<const N: u64>
where
    SlabCheck<N>: SlabSize,
{
    base: NonNull<u8>,
    mask: u32,
    len: u32,
}

impl<const N: u64> SlabPage<N>
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

    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        assert!(layout.size() <= Self::SLOT_BYTES, "{:?}", layout);

        for i in 0..32 {
            let mask_entry = 1 << i;
            if mask_entry & self.mask == 0 {
                self.mask |= mask_entry;
                unsafe {
                    let s = core::slice::from_raw_parts_mut(
                        self.base.as_ptr().offset(i * (Self::SLOT_BYTES as isize)),
                        Self::SLOT_BYTES,
                    );
                    return Ok(NonNull::from(s));
                }
            }
        }
        Err(AllocError)
    }

    fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        assert!(layout.size() <= Self::SLOT_BYTES, "{:?}={:?}", ptr, layout);

        let ptr = ptr.as_ptr() as u64;
        let this = self.base.as_ptr() as u64;

        let offset = (ptr - this) as usize / Self::SLOT_BYTES;
        let mask = 1 << offset;
        self.mask ^= mask;
    }
}

impl<const N: u64> core::fmt::Debug for SlabPage<N>
where
    SlabCheck<N>: SlabSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SlabPage")
            .field("base", &format_args!("{:#x}", self.base.as_ptr() as u64))
            .field("mask", &format_args!("{:#034b}", self.mask))
            .finish()
    }
}

unsafe impl<const N: u64> Allocator for crate::sync::mutex::SpinMutex<SlabPage<N>>
where
    SlabCheck<N>: SlabSize,
{
    fn allocate(
        &self,
        layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        assert!(layout.size() <= SlabPage::<N>::SLOT_BYTES);
        libx64::without_interrupts(|| SlabPage::<N>::allocate(&mut *self.lock(), layout))
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        assert!(layout.size() <= SlabPage::<N>::SLOT_BYTES);
        libx64::without_interrupts(|| SlabPage::<N>::deallocate(&mut *self.lock(), ptr, layout))
    }
}
