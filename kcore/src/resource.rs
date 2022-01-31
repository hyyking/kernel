use alloc::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;

use libx64::paging::{
    entry::Flags,
    frame::{FrameAllocator, FrameError},
    page::{PageMapper, PageRange, TlbFlush},
    Page4Kb, PageCheck, PageSize,
};

use crate::sync::SpinMutex;

pub struct Mapped<T, const P: u64>
where
    PageCheck<P>: PageSize,
{
    resource: T,
    page: PageRange<P>,
}

impl<T, const P: u64> Mapped<T, P>
where
    PageCheck<P>: PageSize,
{
    pub const fn new(resource: T, page: PageRange<P>) -> Self {
        Self { resource, page }
    }
    pub const fn resource(&self) -> &T {
        &self.resource
    }
    pub const fn pages(&self) -> &PageRange<P> {
        &self.page
    }
}

impl<T, const N: u64> Mapped<T, N>
where
    PageCheck<N>: PageSize,
{
    /// # Errors
    ///
    /// Errors if the allocator doesn't have enought frames
    pub fn map<M, A>(&self, mapper: &mut M, alloc: &mut A) -> Result<(), FrameError>
    where
        A: FrameAllocator<N> + FrameAllocator<Page4Kb>,
        M: PageMapper<N>,
    {
        self.page.clone().try_for_each(|page| {
            mapper
                .map(
                    page,
                    alloc.alloc()?,
                    Flags::PRESENT | Flags::RW | Flags::US,
                    alloc,
                )
                .map(TlbFlush::ignore)
        })?;

        libx64::paging::invalidate_tlb();
        Ok(())
    }
}

// TODO: SOUNDNESS, but needed for GlobalAlloc
unsafe impl<T, const P: u64> Sync for Mapped<SpinMutex<T>, P> where PageCheck<P>: PageSize {}
unsafe impl<T, const P: u64> Send for Mapped<SpinMutex<T>, P> where PageCheck<P>: PageSize {}

unsafe impl<T, const P: u64> GlobalAlloc for Mapped<T, P>
where
    T: Allocator,
    PageCheck<P>: PageSize,
{
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        <T as Allocator>::allocate(self.resource(), layout)
            .unwrap_or_else(|_| alloc::alloc::handle_alloc_error(layout))
            .as_mut()
            .as_mut_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        <T as Allocator>::deallocate(self.resource(), NonNull::new_unchecked(ptr), layout);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        <T as Allocator>::allocate_zeroed(self.resource(), layout)
            .unwrap_or_else(|_| alloc::alloc::handle_alloc_error(layout))
            .as_mut()
            .as_mut_ptr()
    }

    unsafe fn realloc(&self, ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, old_layout.align());
        let ptr = NonNull::new_unchecked(ptr);
        match old_layout.size().cmp(&new_layout.size()) {
            core::cmp::Ordering::Less => {
                <T as Allocator>::grow(self.resource(), ptr, old_layout, new_layout)
                    .unwrap_or_else(|_| alloc::alloc::handle_alloc_error(new_layout))
                    .as_mut()
                    .as_mut_ptr()
            }
            core::cmp::Ordering::Greater => {
                <T as Allocator>::shrink(self.resource(), ptr, old_layout, new_layout)
                    .unwrap_or_else(|_| alloc::alloc::handle_alloc_error(new_layout))
                    .as_mut()
                    .as_mut_ptr()
            }
            core::cmp::Ordering::Equal => ptr.as_ptr(),
        }
    }
}

unsafe impl<T, const P: u64> Allocator for Mapped<T, P>
where
    T: Allocator,
    PageCheck<P>: PageSize,
{
    fn allocate(&self, layout: Layout) -> Result<core::ptr::NonNull<[u8]>, AllocError> {
        <T as Allocator>::allocate(self.resource(), layout)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<core::ptr::NonNull<[u8]>, AllocError> {
        <T as Allocator>::allocate_zeroed(self.resource(), layout)
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: Layout) {
        <T as Allocator>::deallocate(self.resource(), ptr, layout);
    }

    unsafe fn grow(
        &self,
        ptr: core::ptr::NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, AllocError> {
        <T as Allocator>::grow(self.resource(), ptr, old_layout, new_layout)
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: core::ptr::NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, AllocError> {
        <T as Allocator>::grow_zeroed(self.resource(), ptr, old_layout, new_layout)
    }

    unsafe fn shrink(
        &self,
        ptr: core::ptr::NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, AllocError> {
        <T as Allocator>::shrink(self.resource(), ptr, old_layout, new_layout)
    }

    fn by_ref(&self) -> &Self
    where
        T: Sized,
    {
        self
    }
}
