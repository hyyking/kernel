use core::ptr::NonNull;

use libx64::{
    address::VirtualAddr,
    paging::{
        frame::{FrameTranslator, PhysicalFrame},
        table::{Level2, Level3, Level4, PageLevel, PageTable},
        NotGiantPageSize, NotHugePageSize, PageCheck, PageSize,
    },
};

pub struct OffsetWalker<const N: u64>
where
    PageCheck<N>: PageSize,
{
    offset: VirtualAddr,
}

impl<const N: u64> OffsetWalker<N>
where
    PageCheck<N>: PageSize,
{
    pub const fn new(offset: VirtualAddr) -> Self {
        debug_assert!(!offset.is_null());
        Self { offset }
    }
    pub unsafe fn translate(&self, frame: PhysicalFrame<N>) -> NonNull<()> {
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
    unsafe fn translate_frame(
        &self,
        frame: PhysicalFrame<N>,
    ) -> NonNull<PageTable<<() as PageLevel>::Next>> {
        self.translate(frame).cast()
    }
}

impl<const N: u64> FrameTranslator<Level4, N> for OffsetWalker<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    unsafe fn translate_frame(
        &self,
        frame: PhysicalFrame<N>,
    ) -> NonNull<PageTable<<Level4 as PageLevel>::Next>> {
        self.translate(frame).cast()
    }
}

impl<const N: u64> FrameTranslator<Level3, N> for OffsetWalker<N>
where
    PageCheck<N>: NotHugePageSize,
{
    #[inline]
    unsafe fn translate_frame(
        &self,
        frame: PhysicalFrame<N>,
    ) -> NonNull<PageTable<<Level3 as PageLevel>::Next>> {
        self.translate(frame).cast()
    }
}

impl<const N: u64> FrameTranslator<Level2, N> for OffsetWalker<N>
where
    PageCheck<N>: NotGiantPageSize,
{
    #[inline]
    unsafe fn translate_frame(
        &self,
        frame: PhysicalFrame<N>,
    ) -> NonNull<PageTable<<Level2 as PageLevel>::Next>> {
        self.translate(frame).cast()
    }
}
