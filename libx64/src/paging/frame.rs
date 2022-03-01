use core::pin::Pin;

use crate::{
    address::PhysicalAddr,
    paging::{
        pretty_pagesize,
        table::{PageLevel, PageTable},
        PageCheck, PageSize,
    },
};

pub trait FrameAllocator<const N: u64>
where
    PageCheck<N>: PageSize,
{
    /// # Errors
    ///
    /// Should error if there are no frames left
    fn alloc(&mut self) -> Result<PhysicalFrame<N>, FrameError>;
}

pub trait FrameTranslator<L, const N: u64>
where
    PageCheck<N>: PageSize,
    L: PageLevel,
{
    /// # Safety
    ///
    /// The caller must uphold that the frame is a valid [`PageEntry`](super::entry::PageEntry)
    /// frame
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> Pin<&'a mut PageTable<L::Next>>;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FrameError {
    UnexpectedHugePage,
    EntryMissing,
    Alloc,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhysicalFrame<const N: u64>
where
    PageCheck<N>: PageSize,
{
    addr: PhysicalAddr,
}

impl<const N: u64> PhysicalFrame<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn containing(addr: PhysicalAddr) -> Self {
        Self {
            addr: addr.align_down(N),
        }
    }

    #[inline]
    #[must_use]
    pub const fn ptr(self) -> PhysicalAddr {
        self.addr
    }
}

impl<const N: u64> core::iter::Step for PhysicalFrame<N>
where
    PageCheck<N>: PageSize,
{
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        usize::try_from((end.addr.as_u64() - start.addr.as_u64()) / N).ok()
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let addr = start.addr + N * u64::try_from(count).ok()?;
        Some(Self::containing(addr))
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let addr = start.addr - N * u64::try_from(count).ok()?;
        Some(Self::containing(addr))
    }
}

impl<const N: u64> core::fmt::Debug for PhysicalFrame<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "PhysicalFrame<{}>({:#x})",
            pretty_pagesize(N),
            self.ptr().as_u64(),
        )
    }
}

pub struct FrameRangeInclusive<const N: u64>(core::ops::RangeInclusive<PhysicalFrame<N>>)
where
    PageCheck<N>: PageSize;

impl<const N: u64> Iterator for FrameRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    type Item = PhysicalFrame<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<const N: u64> core::ops::RangeBounds<PhysicalFrame<N>> for FrameRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    fn start_bound(&self) -> core::ops::Bound<&PhysicalFrame<N>> {
        self.0.start_bound()
    }

    fn end_bound(&self) -> core::ops::Bound<&PhysicalFrame<N>> {
        self.0.end_bound()
    }
}

impl<const N: u64> FrameRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn new(start: PhysicalFrame<N>, end: PhysicalFrame<N>) -> Self {
        Self(core::ops::RangeInclusive::new(start, end))
    }

    #[inline]
    #[must_use]
    pub const fn new_addr(start: PhysicalAddr, end: PhysicalAddr) -> Self {
        Self::new(
            PhysicalFrame::containing(start),
            PhysicalFrame::containing(end),
        )
    }

    #[inline]
    #[must_use]
    pub const fn start(&self) -> PhysicalAddr {
        self.0.start().ptr()
    }

    #[inline]
    #[must_use]
    pub const fn end(&self) -> PhysicalAddr {
        self.0.end().ptr()
    }

    pub fn contains<U>(&self, item: &U) -> bool
    where
        PhysicalFrame<N>: PartialOrd<U>,
        U: ?Sized + PartialOrd<PhysicalFrame<N>>,
    {
        <Self as core::ops::RangeBounds<PhysicalFrame<N>>>::contains(self, item)
    }

    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        ((self.end().as_u64() - self.start().as_u64()) / N) as usize
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    #[must_use]
    pub const fn with_size(start: PhysicalAddr, size: u64) -> Self {
        debug_assert!(size % N == 0, "size must be a multiple of the page size");

        let end = PhysicalFrame::containing(PhysicalAddr::new(start.as_u64() + size));
        let start = PhysicalFrame::containing(PhysicalAddr::new(start.as_u64()));
        Self::new(start, end)
    }
}

impl<const N: u64> core::fmt::Debug for FrameRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "FrameRangeInclusive<{}>({:#x}..{:#x})",
            pretty_pagesize(N),
            self.start().as_u64(),
            self.end().as_u64(),
        )
    }
}

pub struct FrameRange<const N: u64>(core::ops::Range<PhysicalFrame<N>>)
where
    PageCheck<N>: PageSize;

impl<const N: u64> Iterator for FrameRange<N>
where
    PageCheck<N>: PageSize,
{
    type Item = PhysicalFrame<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<const N: u64> core::ops::RangeBounds<PhysicalFrame<N>> for FrameRange<N>
where
    PageCheck<N>: PageSize,
{
    fn start_bound(&self) -> core::ops::Bound<&PhysicalFrame<N>> {
        self.0.start_bound()
    }

    fn end_bound(&self) -> core::ops::Bound<&PhysicalFrame<N>> {
        self.0.end_bound()
    }
}

impl<const N: u64> FrameRange<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn new(start: PhysicalFrame<N>, end: PhysicalFrame<N>) -> Self {
        Self(core::ops::Range { start, end })
    }

    #[inline]
    #[must_use]
    pub const fn new_addr(start: PhysicalAddr, end: PhysicalAddr) -> Self {
        Self::new(
            PhysicalFrame::containing(start),
            PhysicalFrame::containing(end),
        )
    }

    #[inline]
    #[must_use]
    pub const fn start(&self) -> PhysicalAddr {
        self.0.start.ptr()
    }

    #[inline]
    #[must_use]
    pub const fn end(&self) -> PhysicalAddr {
        self.0.end.ptr()
    }

    pub fn contains<U>(&self, item: &U) -> bool
    where
        PhysicalFrame<N>: PartialOrd<U>,
        U: ?Sized + PartialOrd<PhysicalFrame<N>>,
    {
        <Self as core::ops::RangeBounds<PhysicalFrame<N>>>::contains(self, item)
    }

    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        ((self.end().as_u64() - self.start().as_u64()) / N) as usize
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    #[must_use]
    pub const fn with_size(start: PhysicalAddr, size: u64) -> Self {
        debug_assert!(size % N == 0, "size must be a multiple of the page size");

        let end = PhysicalFrame::containing(PhysicalAddr::new(start.as_u64() + size));
        let start = PhysicalFrame::containing(PhysicalAddr::new(start.as_u64()));
        Self::new(start, end)
    }
}

impl<const N: u64> core::fmt::Debug for FrameRange<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "FrameRangeInclusive<{}>({:#x}..{:#x})",
            pretty_pagesize(N),
            self.start().as_u64(),
            self.end().as_u64(),
        )
    }
}

#[cfg(test)]
mod test {
    use crate::paging::Page4Kb;

    use super::*;

    #[test]
    fn iter() {
        fn assert_iter<N: Iterator<Item = PhysicalFrame<Page4Kb>>>(_: N) {}

        assert_iter(FrameRangeInclusive::new(
            PhysicalFrame::containing(PhysicalAddr::new(0)),
            PhysicalFrame::containing(PhysicalAddr::new(0)),
        ));

        assert_iter(FrameRange::new(
            PhysicalFrame::containing(PhysicalAddr::new(0)),
            PhysicalFrame::containing(PhysicalAddr::new(0)),
        ));
    }

    #[test]
    fn inclusive() {
        assert_eq!(
            FrameRangeInclusive::<Page4Kb>::new_addr(
                PhysicalAddr::new(0),
                PhysicalAddr::new(Page4Kb)
            )
            .count(),
            2
        );
        assert_eq!(
            FrameRange::<Page4Kb>::new_addr(PhysicalAddr::new(0), PhysicalAddr::new(Page4Kb))
                .count(),
            1
        );
    }
}
