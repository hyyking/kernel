use crate::{
    address::VirtualAddr,
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, PhysicalFrame},
        invlpg, pretty_pagesize,
        table::Translation,
        Page4Kb, PageCheck, PageSize,
    },
};

use super::table::Level4;

pub trait PageTranslator {
    /// # Errors
    ///
    /// Translations fail if the address has no associated mapping
    fn try_translate(&mut self, addr: VirtualAddr) -> Result<Translation, FrameError>;
}

pub struct TlbFlush<const P: usize>(Page<P>)
where
    PageCheck<P>: PageSize;

impl<const P: usize> TlbFlush<P>
where
    PageCheck<P>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn new(page: Page<P>) -> Self {
        Self(page)
    }

    #[inline]
    pub fn flush(self) {
        invlpg(self.0.ptr());
    }

    #[inline]
    #[allow(clippy::unused_self)]
    pub fn ignore(self) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbMethod {
    Ignore,
    Invalidate,
    FlushAll,
}

pub trait PageMapper<const N: usize>
where
    PageCheck<N>: PageSize,
{
    /// # Safety
    ///
    /// The page must be valid and not have an associated mapper
    unsafe fn from_level4(page: super::PinTableMut<'_, Level4>) -> Self;
    fn level4(&mut self) -> super::PinTableMut<'_, Level4>;

    /// # Errors
    ///
    /// - No more available frames
    fn map<A>(
        &mut self,
        page: Page<N>,
        frame: PhysicalFrame<N>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<TlbFlush<N>, FrameError>
    where
        A: FrameAllocator<Page4Kb>;

    /// # Errors
    ///
    /// Errors if the page is not found
    fn update_flags(&mut self, page: Page<N>, flags: Flags) -> Result<TlbFlush<N>, FrameError>;

    /// # Errors
    ///
    /// Errors if the page is not found
    fn unmap(&mut self, page: Page<N>) -> Result<TlbFlush<N>, FrameError>;

    /// # Errors
    ///
    /// Errors if the page has been miss mapped, or no frames are available in the allocator
    fn id_map<A>(
        &mut self,
        frame: PhysicalFrame<N>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<TlbFlush<N>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        let page = Page::containing(VirtualAddr::new(frame.ptr().as_u64()));
        self.map(page, frame, flags, allocator)
    }

    /// # Errors
    ///
    /// Errors on the first miss-mapped page
    fn map_range<A, P, F>(
        &mut self,
        pages: P,
        frames: F,
        flags: Flags,
        allocator: &mut A,
        method: TlbMethod,
    ) -> Result<(), FrameError>
    where
        A: FrameAllocator<Page4Kb>,
        P: Iterator<Item = Page<N>> + ExactSizeIterator,
        F: Iterator<Item = PhysicalFrame<N>> + ExactSizeIterator,
    {
        assert_eq!(
            pages.len(),
            frames.len(),
            "map_range requires pages and frames to be the same length"
        );
        let flushfn = match method {
            TlbMethod::FlushAll => TlbFlush::flush,
            _ => TlbFlush::ignore,
        };

        pages
            .zip(frames)
            .try_for_each(|(page, frame)| self.map(page, frame, flags, allocator).map(flushfn))?;

        if method == TlbMethod::Invalidate {
            super::invalidate_tlb();
        }

        Ok(())
    }

    /// # Errors
    ///
    /// This method errors on the first miss-mapped page or if no frames are available in the allocator
    #[allow(clippy::trait_duplication_in_bounds)]
    fn map_range_alloc<A, P>(
        &mut self,
        mut pages: P,
        flags: Flags,
        allocator: &mut A,
        method: TlbMethod,
    ) -> Result<(), FrameError>
    where
        A: FrameAllocator<Page4Kb> + FrameAllocator<N>,
        P: Iterator<Item = Page<N>>,
    {
        let flushfn = match method {
            TlbMethod::FlushAll => TlbFlush::flush,
            _ => TlbFlush::ignore,
        };

        pages.try_for_each(|page| {
            let frame = <A as FrameAllocator<N>>::alloc(allocator)?;
            self.map(page, frame, flags, allocator).map(flushfn)
        })?;

        if method == TlbMethod::Invalidate {
            super::invalidate_tlb();
        }

        Ok(())
    }

    /// # Errors
    /// This method errors on the first miss-mapped page
    fn id_map_range<A, F>(
        &mut self,
        mut frames: F,
        flags: Flags,
        allocator: &mut A,
        method: TlbMethod,
    ) -> Result<(), FrameError>
    where
        A: FrameAllocator<Page4Kb>,
        F: Iterator<Item = PhysicalFrame<N>>,
    {
        let flushfn = match method {
            TlbMethod::FlushAll => TlbFlush::flush,
            _ => TlbFlush::ignore,
        };

        frames.try_for_each(|frame| self.id_map(frame, flags, allocator).map(flushfn))?;

        if method == TlbMethod::Invalidate {
            super::invalidate_tlb();
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Page<const N: usize>
where
    PageCheck<N>: PageSize,
{
    addr: VirtualAddr,
}

impl<const N: usize> core::iter::Step for Page<N>
where
    PageCheck<N>: PageSize,
{
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some((end.addr.as_usize() - start.addr.as_usize()) / N)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let addr = start.addr.as_usize().checked_add(N * count)?;
        Some(Self::containing(VirtualAddr::new(addr as u64)))
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let addr = start.addr.as_usize().checked_sub(N * count)?;
        Some(Self::containing(VirtualAddr::new(addr as u64)))
    }
}

impl<const N: usize> Page<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn containing(addr: VirtualAddr) -> Self {
        Self {
            addr: addr.align_down(N as u64),
        }
    }

    #[inline]
    #[must_use]
    pub fn containing_ptr<T>(addr: *const T) -> Self {
        Self {
            addr: VirtualAddr::from_ptr(addr).align_down(N as u64),
        }
    }

    #[inline]
    #[must_use]
    /// # Panics
    /// Compile time panics if the page size isn't valid
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
    pub const fn ptr(self) -> VirtualAddr {
        self.addr
    }

    #[inline]
    #[must_use]
    pub const fn end_ptr(self) -> VirtualAddr {
        VirtualAddr::new((self.addr.as_usize() + N) as u64)
    }
}

// impl Page<Page4Kb> {}
// impl Page<Page2Mb> {}
// impl Page<Page1Gb> {}

impl<const N: usize> core::fmt::Debug for Page<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Page<{}>({:#x})",
            pretty_pagesize(N),
            self.ptr().as_u64(),
        )
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PageRangeInclusive<const N: usize>(core::ops::RangeInclusive<Page<N>>)
where
    PageCheck<N>: PageSize;

impl<const N: usize> Iterator for PageRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    type Item = Page<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<const N: usize> ExactSizeIterator for PageRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    fn len(&self) -> usize {
        self.len()
    }
}

impl<const N: usize> core::ops::RangeBounds<Page<N>> for PageRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    fn start_bound(&self) -> core::ops::Bound<&Page<N>> {
        self.0.start_bound()
    }

    fn end_bound(&self) -> core::ops::Bound<&Page<N>> {
        self.0.end_bound()
    }
}

impl<const N: usize> PageRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn new(start: Page<N>, end: Page<N>) -> Self {
        Self(core::ops::RangeInclusive::new(start, end))
    }

    #[inline]
    #[must_use]
    pub const fn new_addr(start: VirtualAddr, end: VirtualAddr) -> Self {
        Self::new(Page::containing(start), Page::containing(end))
    }

    #[inline]
    #[must_use]
    pub const fn start(&self) -> VirtualAddr {
        self.0.start().ptr()
    }

    #[inline]
    #[must_use]
    pub const fn end(&self) -> VirtualAddr {
        self.0.end().ptr()
    }

    pub fn contains<U>(&self, item: &U) -> bool
    where
        Page<N>: PartialOrd<U>,
        U: ?Sized + PartialOrd<Page<N>>,
    {
        <Self as core::ops::RangeBounds<Page<N>>>::contains(self, item)
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
    pub const fn with_size(start: VirtualAddr, size: u64) -> Self {
        debug_assert!(
            size % N as u64 == 0,
            "size must be a multiple of the page size"
        );

        let end = Page::containing(VirtualAddr::new(start.as_u64() + size));
        let start = Page::containing(VirtualAddr::new(start.as_u64()));
        Self::new(start, end)
    }
}

impl<const N: usize> core::fmt::Debug for PageRangeInclusive<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "PageRangeInclusive<{}>({:#x}..{:#x})",
            pretty_pagesize(N),
            self.start().as_u64(),
            self.end().as_u64(),
        )
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PageRange<const N: usize>(core::ops::Range<Page<N>>)
where
    PageCheck<N>: PageSize;

impl<const N: usize> Iterator for PageRange<N>
where
    PageCheck<N>: PageSize,
{
    type Item = Page<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<const N: usize> ExactSizeIterator for PageRange<N>
where
    PageCheck<N>: PageSize,
{
    fn len(&self) -> usize {
        self.len()
    }
}

impl<const N: usize> core::ops::RangeBounds<Page<N>> for PageRange<N>
where
    PageCheck<N>: PageSize,
{
    fn start_bound(&self) -> core::ops::Bound<&Page<N>> {
        self.0.start_bound()
    }

    fn end_bound(&self) -> core::ops::Bound<&Page<N>> {
        self.0.end_bound()
    }
}

impl<const N: usize> PageRange<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn new(start: Page<N>, end: Page<N>) -> Self {
        Self(core::ops::Range { start, end })
    }

    #[inline]
    #[must_use]
    pub const fn new_addr(start: VirtualAddr, end: VirtualAddr) -> Self {
        Self::new(Page::containing(start), Page::containing(end))
    }

    #[inline]
    #[must_use]
    pub const fn start(&self) -> VirtualAddr {
        self.0.start.ptr()
    }

    #[inline]
    #[must_use]
    pub const fn end(&self) -> VirtualAddr {
        self.0.end.ptr()
    }

    pub fn contains<U>(&self, item: &U) -> bool
    where
        Page<N>: PartialOrd<U>,
        U: ?Sized + PartialOrd<Page<N>>,
    {
        <Self as core::ops::RangeBounds<Page<N>>>::contains(self, item)
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
    pub const fn with_size(start: VirtualAddr, size: u64) -> Self {
        debug_assert!(
            size % N as u64 == 0,
            "size must be a multiple of the page size"
        );

        let end = Page::containing(VirtualAddr::new(start.as_u64() + size + 1));
        let start = Page::containing(VirtualAddr::new(start.as_u64()));
        Self::new(start, end)
    }
}

impl<const N: usize> core::fmt::Debug for PageRange<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "PageRange<{}>({:#x}..{:#x})",
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
        fn assert_iter<N: Iterator<Item = Page<Page4Kb>>>(_: N) {}

        assert_iter(PageRangeInclusive::new(
            Page::containing(VirtualAddr::new(0)),
            Page::containing(VirtualAddr::new(0)),
        ));

        assert_iter(PageRange::new(
            Page::containing(VirtualAddr::new(0)),
            Page::containing(VirtualAddr::new(0)),
        ));
    }

    #[test]
    fn inclusive() {
        assert_eq!(
            PageRangeInclusive::<Page4Kb>::new_addr(
                VirtualAddr::new(0),
                VirtualAddr::new(Page4Kb as u64)
            )
            .count(),
            2
        );
        assert_eq!(
            PageRange::<Page4Kb>::new_addr(VirtualAddr::new(0), VirtualAddr::new(Page4Kb as u64))
                .count(),
            1
        );
    }
}
