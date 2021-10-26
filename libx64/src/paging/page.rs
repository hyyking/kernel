use crate::{
    address::VirtualAddr,
    paging::{entry::Flags, frame::FrameError, PageCheck, PageSize},
};

use super::{
    frame::{FrameAllocator, PhysicalFrame},
    Page4Kb,
};

pub trait PageMapper<A, const N: u64>
where
    A: FrameAllocator<Page4Kb> + FrameAllocator<N>,
    PageCheck<N>: PageSize,
{
    fn map(
        &mut self,
        page: Page<N>,
        frame: PhysicalFrame<N>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<(), FrameError>;
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Page<const N: u64>
where
    PageCheck<N>: PageSize,
{
    addr: VirtualAddr,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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
        if addr.as_u64() > (self.end.as_u64() - 1) {
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
    pub const fn new(start: VirtualAddr, end: VirtualAddr) -> Self {
        Self { start, end, at: 0 }
    }

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
    pub const fn containing(addr: VirtualAddr) -> Self {
        Self {
            addr: addr.align_down(N),
        }
    }

    pub const fn ptr(self) -> VirtualAddr {
        self.addr
    }
}

impl<const N: u64> core::fmt::Debug for Page<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page")
            .field("size", &N)
            .field("ptr", &format_args!("{:#x}", &self.addr.as_u64()))
            .finish()
    }
}
