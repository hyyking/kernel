use core::{pin::Pin, ptr::NonNull};

use libx64::{
    address::VirtualAddr,
    paging::{
        frame::{FrameTranslator, PhysicalFrame},
        table::{Level2, Level3, Level4, PageLevel, PageTable},
        NotGiantPageSize, NotHugePageSize, PageCheck, PageSize,
    },
};

pub(crate) struct OffsetWalker<const N: u64>
where
    PageCheck<N>: PageSize,
{
    offset: VirtualAddr,
}

impl<const N: u64> OffsetWalker<N>
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

impl<const N: u64> FrameTranslator<(), N> for OffsetWalker<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> Pin<&'a mut PageTable<<() as PageLevel>::Next>> {
        Pin::new_unchecked(self.translate(frame).cast().as_mut())
    }
}

impl<const N: u64> FrameTranslator<Level4, N> for OffsetWalker<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> Pin<&'a mut PageTable<<Level4 as PageLevel>::Next>> {
        Pin::new_unchecked(self.translate(frame).cast().as_mut())
    }
}

impl<const N: u64> FrameTranslator<Level3, N> for OffsetWalker<N>
where
    PageCheck<N>: NotHugePageSize,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> Pin<&'a mut PageTable<<Level3 as PageLevel>::Next>> {
        Pin::new_unchecked(self.translate(frame).cast().as_mut())
    }
}

impl<const N: u64> FrameTranslator<Level2, N> for OffsetWalker<N>
where
    PageCheck<N>: NotGiantPageSize,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> Pin<&'a mut PageTable<<Level2 as PageLevel>::Next>> {
        Pin::new_unchecked(self.translate(frame).cast().as_mut())
    }
}
