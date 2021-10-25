use core::ptr::NonNull;

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
    unsafe fn translate_frame(&self, frame: PhysicalFrame<N>) -> NonNull<PageTable<L::Next>>;
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
    pub const fn containing(addr: PhysicalAddr) -> Self {
        Self {
            addr: addr.align_down(N),
        }
    }

    pub const fn ptr(self) -> PhysicalAddr {
        self.addr
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
