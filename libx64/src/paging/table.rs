use core::ptr::NonNull;

use crate::{
    address::{PhysicalAddr, VirtualAddr},
    control::CR3,
    paging::{
        entry::PageEntry,
        frame::{FrameError, FrameKind, FrameTranslator},
        NotGiantPageSize, NotHugePageSize, Page4Kb, PageCheck, PageSize,
    },
};

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PageTable<LEVEL: PageLevel> {
    entries: [PageEntry<LEVEL>; 512],
    _m: core::marker::PhantomData<LEVEL>,
}

impl PageTable<Level4> {
    pub fn new(cr: CR3, translator: &dyn FrameTranslator<(), Page4Kb>) -> NonNull<Self> {
        unsafe { translator.translate_frame(cr.frame()) }
    }

    pub fn walk_next<const P: u64>(
        &self,
        cr: &PageEntry<Level4>,
        translator: &dyn FrameTranslator<Level4, Page4Kb>,
    ) -> Result<NonNull<PageTable<Level3>>, FrameError>
    where
        PageCheck<P>: PageSize,
    {
        let frame = match cr.frame()? {
            FrameKind::Normal(frame) => frame,
            FrameKind::Huge(_) => return Err(FrameError::UnexpectedHugePage),
        };
        unsafe { Ok(translator.translate_frame(frame)) }
    }
}

impl PageTable<Level3> {
    pub fn walk_next<const P: u64>(
        &self,
        cr: &PageEntry<Level3>,
        translator: &dyn FrameTranslator<Level3, Page4Kb>,
    ) -> Result<Option<NonNull<PageTable<Level2>>>, FrameError>
    where
        PageCheck<P>: NotGiantPageSize,
    {
        let frame = match cr.frame()? {
            FrameKind::Normal(frame) => frame,
            FrameKind::Huge(_) => return Ok(None),
        };
        unsafe { Ok(Some(translator.translate_frame(frame))) }
    }
}

impl PageTable<Level2> {
    pub fn walk_next<const P: u64>(
        &self,
        cr: &PageEntry<Level2>,
        translator: &dyn FrameTranslator<Level2, Page4Kb>,
    ) -> Result<Option<NonNull<PageTable<Level1>>>, FrameError>
    where
        PageCheck<P>: NotHugePageSize,
    {
        let frame = match cr.frame()? {
            FrameKind::Normal(frame) => frame,
            FrameKind::Huge(_) => return Ok(None),
        };
        unsafe { Ok(Some(translator.translate_frame(frame))) }
    }
}

impl<LEVEL: PageLevel> PageTable<LEVEL> {
    pub unsafe fn translate_addr(
        &self,
        idx: PageTableIndex<LEVEL>,
        virt: VirtualAddr,
    ) -> Result<PhysicalAddr, FrameError> {
        match self[idx].frame()? {
            FrameKind::Normal(frame) => Ok(frame.ptr() + u64::from(virt.page_offset())),
            FrameKind::Huge(ptr) => Ok(ptr + u64::from(virt.page_offset())),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableIndex<T: PageLevel> {
    idx: usize,
    _m: core::marker::PhantomData<T>,
}

impl<T: PageLevel> PageTableIndex<T> {
    pub fn new_truncate(value: u16) -> Self {
        Self {
            idx: (value as usize) % 512,
            _m: core::marker::PhantomData,
        }
    }
}

impl<T: PageLevel> core::fmt::Debug for PageTableIndex<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PageTableIndex")
            .field("idx", &self.idx)
            .field("level", &core::any::type_name::<T>())
            .finish()
    }
}

impl<LEVEL: PageLevel> core::ops::Index<PageTableIndex<LEVEL>> for PageTable<LEVEL> {
    type Output = PageEntry<LEVEL>;

    fn index(&self, idx: PageTableIndex<LEVEL>) -> &Self::Output {
        &self.entries[idx.idx]
    }
}

impl<LEVEL: PageLevel> core::ops::IndexMut<PageTableIndex<LEVEL>> for PageTable<LEVEL> {
    fn index_mut(&mut self, idx: PageTableIndex<LEVEL>) -> &mut Self::Output {
        &mut self.entries[idx.idx]
    }
}

pub trait PageLevel {
    type Next: PageLevel;
    type Prev: PageLevel;
    const VALUE: u64;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Level1;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Level2;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Level3;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Level4;

impl PageLevel for () {
    const VALUE: u64 = 0;
    type Next = Level4;
    type Prev = !;
}

impl PageLevel for Level4 {
    const VALUE: u64 = 4;
    type Next = Level3;
    type Prev = ();
}

impl PageLevel for Level3 {
    const VALUE: u64 = 3;
    type Next = Level2;
    type Prev = Level4;
}

impl PageLevel for Level2 {
    const VALUE: u64 = 2;
    type Next = Level1;
    type Prev = Level3;
}

impl PageLevel for Level1 {
    const VALUE: u64 = 1;
    type Next = !;
    type Prev = Level2;
}

impl PageLevel for ! {
    const VALUE: u64 = 0;
    type Next = !;
    type Prev = Level1;
}
