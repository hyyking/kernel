use core::pin::Pin;

use crate::{
    address::PhysicalAddr,
    paging::{
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

#[derive(Clone, Copy, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FrameRange<const N: u64>
where
    PageCheck<N>: PageSize,
{
    start: PhysicalAddr,
    end: PhysicalAddr,
    at: u64,
}

impl<const N: u64> Iterator for FrameRange<N>
where
    PageCheck<N>: PageSize,
{
    type Item = PhysicalFrame<N>;

    fn next(&mut self) -> Option<Self::Item> {
        let addr = self.start.align_down(N) + (self.at * N);
        if addr.as_u64() > self.end.as_u64() {
            return None;
        }
        self.at += 1;
        Some(PhysicalFrame::containing(addr))
    }
}

impl<const N: u64> FrameRange<N>
where
    PageCheck<N>: PageSize,
{
    #[inline]
    #[must_use]
    pub const fn new(start: PhysicalFrame<N>, end: PhysicalFrame<N>) -> Self {
        Self {
            start: start.ptr(),
            end: end.ptr(),
            at: 0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn new_addr(start: PhysicalAddr, end: PhysicalAddr) -> Self {
        Self { start, end, at: 0 }
    }

    #[inline]
    #[must_use]
    pub const fn start(&self) -> PhysicalAddr {
        self.start
    }

    #[inline]
    #[must_use]
    pub const fn end(&self) -> PhysicalAddr {
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
    pub const fn with_size(start: PhysicalAddr, size: u64) -> Self {
        debug_assert!(size % N == 0, "size must be a multiple of the page size");
        let end = PhysicalAddr::new(start.as_u64() + size - 1);
        Self { start, end, at: 0 }
    }
}

impl<const N: u64> core::fmt::Debug for PhysicalFrame<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PhysicalFrame")
            .field("size", &N)
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
            FrameRange::<Page4Kb>::new(PhysicalAddr::new(0), PhysicalAddr::new(Page4Kb)).count(),
            2
        )
    }
}
