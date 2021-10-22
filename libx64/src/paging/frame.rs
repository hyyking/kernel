use core::ptr::NonNull;

use crate::{
    address::PhysicalAddr,
    paging::{
        table::{PageLevel, PageTable},
        Page4Kb, PageCheck, PageSize,
    },
};

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
}

pub enum FrameKind {
    Normal(PhysicalFrame<Page4Kb>),
    Huge(PhysicalAddr),
}

#[derive(Debug, Clone, Copy)]
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

impl FrameKind {
    /// Returns `true` if the frame kind is [`Huge`].
    ///
    /// [`Huge`]: FrameKind::Huge
    pub fn is_huge(&self) -> bool {
        matches!(self, Self::Huge(..))
    }

    pub unsafe fn into_level3_huge_page(
        self,
        _page: &super::table::PageTable<super::table::Level3>,
    ) -> Option<PhysicalFrame<{ super::Page1Gb }>> {
        match self {
            Self::Normal(_) => None,
            Self::Huge(addr) => Some(PhysicalFrame::containing(addr)),
        }
    }

    pub unsafe fn into_level2_huge_page(
        self,
        _page: &super::table::PageTable<super::table::Level2>,
    ) -> Option<PhysicalFrame<{ super::Page2Mb }>> {
        match self {
            Self::Normal(_) => None,
            Self::Huge(addr) => Some(PhysicalFrame::containing(addr)),
        }
    }
}
