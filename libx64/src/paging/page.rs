use crate::{
    address::VirtualAddr,
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, PhysicalFrame},
        invlpg, Page1Gb, Page2Mb, Page4Kb, PageCheck, PageSize,
    },
};

pub struct TlbFlush<const P: u64>(Page<P>)
where
    PageCheck<P>: PageSize;

impl<const P: u64> TlbFlush<P>
where
    PageCheck<P>: PageSize,
{
    pub fn new(page: Page<P>) -> Self {
        Self(page)
    }

    #[inline]
    pub fn flush(self) {
        invlpg(self.0.ptr());
    }

    #[inline]
    pub fn ignore(self) {}
}

pub trait PageMapper<const N: u64>
where
    PageCheck<N>: PageSize,
{
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

    fn update_flags(&mut self, page: Page<N>, flags: Flags) -> Result<TlbFlush<N>, FrameError>;

    fn unmap(&mut self, page: Page<N>) -> Result<TlbFlush<N>, FrameError>;

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
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Page<const N: u64>
where
    PageCheck<N>: PageSize,
{
    addr: VirtualAddr,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PageRange<const N: u64>
where
    PageCheck<N>: PageSize,
{
    start: VirtualAddr,
    end: VirtualAddr,
    at: u64,
}

impl<const N: u64> Iterator for PageRange<N>
where
    PageCheck<N>: PageSize,
{
    type Item = Page<N>;

    fn next(&mut self) -> Option<Self::Item> {
        let addr = self.start.align_down(N) + (self.at * N);
        if addr.as_u64() > self.end.as_u64() {
            return None;
        }
        self.at += 1;
        Some(Page::containing(addr))
    }
}

impl<const N: u64> PageRange<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn new(start: Page<N>, end: Page<N>) -> Self {
        Self {
            start: start.ptr(),
            end: end.ptr(),
            at: 0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn new_addr(start: VirtualAddr, end: VirtualAddr) -> Self {
        Self { start, end, at: 0 }
    }

    #[inline]
    #[must_use]
    pub const fn start(&self) -> VirtualAddr {
        self.start
    }

    #[inline]
    #[must_use]
    pub const fn end(&self) -> VirtualAddr {
        self.end
    }

    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        ((self.end.as_u64() - self.start.as_u64()) / N) as usize
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    #[must_use]
    pub const fn with_size(start: VirtualAddr, size: u64) -> Self {
        debug_assert!(size % N == 0, "size must be a multiple of the page size");
        let end = VirtualAddr::new(start.as_u64() + size);
        Self { start, end, at: 0 }
    }
}

impl<const N: u64> Page<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn containing(addr: VirtualAddr) -> Self {
        Self {
            addr: addr.align_down(N),
        }
    }

    #[inline]
    #[must_use]
    pub const fn ptr(self) -> VirtualAddr {
        self.addr
    }

    pub const fn end_ptr(self) -> VirtualAddr {
        VirtualAddr::new(self.addr.as_u64() + N)
    }
}

// impl Page<Page4Kb> {}
// impl Page<Page2Mb> {}
// impl Page<Page1Gb> {}

impl<const N: u64> core::fmt::Debug for Page<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page")
            .field("size", &{
                #[allow(non_upper_case_globals)]
                match N {
                    Page4Kb => "4Kb",
                    Page2Mb => "2Mb",
                    Page1Gb => "1Gb",
                    _ => "UKN",
                }
            })
            .field("ptr", &format_args!("{:#x}", &self.addr.as_u64()))
            .finish()
    }
}

#[cfg(test)]
mod test {
    use crate::paging::Page4Kb;

    use super::*;
    #[test]
    fn inclusive() {
        assert_eq!(
            PageRange::<Page4Kb>::new(VirtualAddr::new(0), VirtualAddr::new(Page4Kb)).count(),
            2
        )
    }
}
