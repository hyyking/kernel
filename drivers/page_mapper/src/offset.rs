use core::{pin::Pin, ptr::NonNull};

use libx64::{
    address::VirtualAddr,
    paging::{
        frame::{FrameTranslator, PhysicalFrame},
        table::{Level2, Level3, Level4, PageLevel},
        NotGiantPageSize, NotHugePageSize, PageCheck, PageSize, PinTableMut,
    },
};

pub(crate) struct OffsetWalker<const N: usize>
where
    PageCheck<N>: PageSize,
{
    offset: VirtualAddr,
}

impl<const N: usize> OffsetWalker<N>
where
    PageCheck<N>: PageSize,
{
    pub(crate) const fn new(offset: VirtualAddr) -> Self {
        debug_assert!(!offset.is_null());
        Self { offset }
    }

    pub(crate) fn translate(&self, frame: PhysicalFrame<N>) -> NonNull<()> {
        (self.offset + frame.ptr().as_u64())
            .ptr()
            .expect("null frame pointer and offset")
    }
}

impl<const N: usize> FrameTranslator<(), N> for OffsetWalker<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> PinTableMut<'a, <() as PageLevel>::Next> {
        Pin::new_unchecked(self.translate(frame).cast().as_mut())
    }
}

impl<const N: usize> FrameTranslator<Level4, N> for OffsetWalker<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> PinTableMut<'a, <Level4 as PageLevel>::Next> {
        Pin::new_unchecked(self.translate(frame).cast().as_mut())
    }
}

impl<const N: usize> FrameTranslator<Level3, N> for OffsetWalker<N>
where
    PageCheck<N>: NotHugePageSize,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> PinTableMut<'a, <Level3 as PageLevel>::Next> {
        Pin::new_unchecked(self.translate(frame).cast().as_mut())
    }
}

impl<const N: usize> FrameTranslator<Level2, N> for OffsetWalker<N>
where
    PageCheck<N>: NotGiantPageSize,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> PinTableMut<'a, <Level2 as PageLevel>::Next> {
        Pin::new_unchecked(self.translate(frame).cast().as_mut())
    }
}
