use core::pin::Pin;

use crate::{
    address::PhysicalAddr,
    paging::{
        pretty_pagesize,
        table::{PageLevel, PageTable},
        PageCheck, PageSize,
    },
};

pub trait FrameAllocator<const N: usize>
where
    PageCheck<N>: PageSize,
{
    /// # Errors
    ///
    /// Should error if there are no frames left
    fn alloc(&mut self) -> Result<PhysicalFrame<N>, FrameError>;
}

pub trait FrameTranslator<L, const N: usize>
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

pub struct IdentityTranslator;

impl<L: PageLevel, const N: usize> FrameTranslator<L, N> for IdentityTranslator where PageCheck<N>: PageSize {
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<N>,
    ) -> Pin<&'a mut PageTable<<L as PageLevel>::Next>> {
        Pin::new_unchecked(frame.ptr().ptr::<PageTable<<L as PageLevel>::Next>>().unwrap().as_mut())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FrameError {
    UnexpectedHugePage,
    EntryMissing,
    Alloc,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhysicalFrame<const N: usize>
where
    PageCheck<N>: PageSize,
{
    addr: PhysicalAddr,
}

impl<const N: usize> PhysicalFrame<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn containing(addr: PhysicalAddr) -> Self {
        Self {
            addr: addr.align_down(N as u64),
        }
    }

    #[inline]
    #[must_use]
    pub fn containing_ptr<T>(addr: *const T) -> Self {
        Self {
            addr: PhysicalAddr::from_ptr(addr).align_down(N as u64),
        }
    }

    #[inline]
    #[must_use]
    /// # Panics
    ///
    /// Compile time panic if N is not `PageSize` (it is trait bound in this impl but the compiler can't list all alternatives for us)
    pub const fn alloc_layout() -> core::alloc::Layout {
        match N {
            super::Page4Kb => core::alloc::Layout::new::<[u8; 4 * crate::units::KB]>(),
            super::Page2Mb => core::alloc::Layout::new::<[u8; 2 * crate::units::MB]>(),
            super::Page1Gb => core::alloc::Layout::new::<[u8; crate::units::GB]>(),
            _ => panic!("unsupported page size"),
        }
    }

    #[inline]
    #[must_use]
    pub const fn ptr(self) -> PhysicalAddr {
        self.addr
    }
}

impl<const N: usize> core::iter::Step for PhysicalFrame<N>
where
    PageCheck<N>: PageSize,
{
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some((end.addr.as_usize() - start.addr.as_usize()) / N)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let addr = start.addr.as_usize().checked_add(N * count)?;
        Some(Self::containing(PhysicalAddr::new(addr as u64)))
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let addr = start.addr.as_usize().checked_sub(N * count)?;
        Some(Self::containing(PhysicalAddr::new(addr as u64)))
    }
}

impl<const N: usize> core::fmt::Debug for PhysicalFrame<N>
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

#[derive(Clone, Eq, PartialEq)]
pub struct FrameRangeInclusive<const N: usize>(core::ops::RangeInclusive<PhysicalFrame<N>>)
where
    PageCheck<N>: PageSize;

impl<const N: usize> Iterator for FrameRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    type Item = PhysicalFrame<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<const N: usize> ExactSizeIterator for FrameRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    fn len(&self) -> usize {
        self.len()
    }
}

impl<const N: usize> core::ops::RangeBounds<PhysicalFrame<N>> for FrameRangeInclusive<N>
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

impl<const N: usize> FrameRangeInclusive<N>
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
        (self.end().as_usize() - self.start().as_usize()) / N
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    #[must_use]
    pub const fn with_size(start: PhysicalAddr, size: u64) -> Self {
        debug_assert!(
            size % N as u64 == 0,
            "size must be a multiple of the page size"
        );
        let end = PhysicalFrame::containing(PhysicalAddr::new(start.as_u64() + size));
        let start = PhysicalFrame::containing(PhysicalAddr::new(start.as_u64()));
        Self::new(start, end)
    }
}

impl<const N: usize> core::fmt::Debug for FrameRangeInclusive<N>
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

#[derive(Clone, Eq, PartialEq)]
pub struct FrameRange<const N: usize>(core::ops::Range<PhysicalFrame<N>>)
where
    PageCheck<N>: PageSize;

impl<const N: usize> Iterator for FrameRange<N>
where
    PageCheck<N>: PageSize,
{
    type Item = PhysicalFrame<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<const N: usize> ExactSizeIterator for FrameRange<N>
where
    PageCheck<N>: PageSize,
{
    fn len(&self) -> usize {
        self.len()
    }
}

impl<const N: usize> core::ops::RangeBounds<PhysicalFrame<N>> for FrameRange<N>
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

impl<const N: usize> FrameRange<N>
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
        (self.end().as_usize() - self.start().as_usize()) / N
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    #[must_use]
    pub const fn with_size(start: PhysicalAddr, size: u64) -> Self {
        debug_assert!(
            size % N as u64 == 0,
            "size must be a multiple of the page size"
        );

        let end = PhysicalFrame::containing(PhysicalAddr::new(start.as_u64() + size + 1));
        let start = PhysicalFrame::containing(PhysicalAddr::new(start.as_u64()));
        Self::new(start, end)
    }
}

impl<const N: usize> core::fmt::Debug for FrameRange<N>
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
                PhysicalAddr::new(Page4Kb as u64)
            )
            .count(),
            2
        );
        assert_eq!(
            FrameRange::<Page4Kb>::new_addr(
                PhysicalAddr::new(0),
                PhysicalAddr::new(Page4Kb as u64)
            )
            .count(),
            1
        );
    }
}
