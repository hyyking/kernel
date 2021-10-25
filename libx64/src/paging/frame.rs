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

pub trait FrameTranslator<LEVEL, const N: u64>
where
    PageCheck<N>: PageSize,
    LEVEL: PageLevel,
{
    unsafe fn translate_frame(&self, frame: PhysicalFrame<N>) -> NonNull<PageTable<LEVEL::Next>>;
}

#[derive(Debug)]
pub enum FrameError {
    UnexpectedHugePage,
    EntryMissing,
    Alloc,
}

#[derive(Clone, Copy)]
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
