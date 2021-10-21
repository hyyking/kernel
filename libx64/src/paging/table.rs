use core::ptr::NonNull;

use crate::{
    address::{PhysicalAddr, VirtualAddr},
    control::CR3,
    paging::{
        entry::PageEntry,
        frame::{FrameError, FrameTranslator},
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
        translator: &dyn FrameTranslator<Level4, P>,
    ) -> Result<NonNull<PageTable<Level3>>, FrameError>
    where
        PageCheck<P>: PageSize,
    {
        unsafe { Ok(translator.translate_frame(cr.frame()?)) }
    }
}

impl PageTable<Level3> {
    pub fn walk_next<const P: u64>(
        &self,
        cr: &PageEntry<Level3>,
        translator: &dyn FrameTranslator<Level3, P>,
    ) -> Result<NonNull<PageTable<Level2>>, FrameError>
    where
        PageCheck<P>: NotGiantPageSize,
    {
        unsafe { Ok(translator.translate_frame(cr.frame()?)) }
    }
}

impl PageTable<Level2> {
    pub fn walk_next<const P: u64>(
        &self,
        cr: &PageEntry<Level2>,
        translator: &dyn FrameTranslator<Level2, P>,
    ) -> Result<NonNull<PageTable<Level1>>, FrameError>
    where
        PageCheck<P>: NotHugePageSize,
    {
        unsafe { Ok(translator.translate_frame(cr.frame()?)) }
    }
}

impl PageTable<Level1> {
    pub fn translate_addr(&self, virt: VirtualAddr) -> PhysicalAddr {
        let addr = self[virt.page_table_index(Level1)]
            .frame::<Page4Kb>()
            .unwrap();
        addr.ptr() + u64::from(virt.page_offset())
    }
}

#[derive(Debug, Clone, Copy)]
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

pub struct Level1;
pub struct Level2;
pub struct Level3;
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
