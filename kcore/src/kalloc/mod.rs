use core::ptr::NonNull;

use alloc::{
    alloc::{AllocError, Allocator, Layout},
    sync::Arc,
};

pub mod slab;

#[derive(Debug)]
pub struct SharedAllocator<A> {
    inner: Arc<A>,
}

impl<A> Clone for SharedAllocator<A> {
    fn clone(&self) -> Self {
        SharedAllocator {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<A> SharedAllocator<A> {
    pub fn new(alloc: A) -> Self {
        Self {
            inner: Arc::new(alloc),
        }
    }
}

unsafe impl<A> Allocator for SharedAllocator<A>
where
    A: Allocator,
{
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        <A as Allocator>::allocate(&*self.inner, layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        <A as Allocator>::deallocate(&*self.inner, ptr, layout)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        <A as Allocator>::allocate_zeroed(&*self.inner, layout)
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        <A as Allocator>::grow(&*self.inner, ptr, old_layout, new_layout)
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        <A as Allocator>::grow_zeroed(&*self.inner, ptr, old_layout, new_layout)
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        <A as Allocator>::shrink(&*self.inner, ptr, old_layout, new_layout)
    }
}
