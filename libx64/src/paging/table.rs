use core::pin::Pin;

use crate::{
    address::{PhysicalAddr, VirtualAddr},
    control::CR3,
    paging::{
        entry::{MappedLevel2Page, MappedLevel3Page, PageEntry},
        frame::{FrameError, FrameTranslator, PhysicalFrame},
        Page1Gb, Page2Mb, Page4Kb,
    },
};

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PageTable<LEVEL: PageLevel> {
    entries: [PageEntry<LEVEL>; 512],
    _m: core::marker::PhantomData<LEVEL>,
    _p: core::marker::PhantomPinned,
}

impl<LEVEL: PageLevel> PageTable<LEVEL> {
    pub fn index_pin(self: Pin<&Self>, idx: PageTableIndex<LEVEL>) -> Pin<&PageEntry<LEVEL>> {
        unsafe { self.map_unchecked(|page| &page[idx]) }
    }
    pub fn index_pin_mut(
        self: Pin<&mut Self>,
        idx: PageTableIndex<LEVEL>,
    ) -> Pin<&mut PageEntry<LEVEL>> {
        unsafe { self.map_unchecked_mut(|page| &mut page[idx]) }
    }
}

impl PageTable<Level4> {
    pub fn new<'a>(cr: CR3, translator: &dyn FrameTranslator<(), Page4Kb>) -> Pin<&'a mut Self> {
        unsafe { translator.translate_frame(cr.frame()) }
    }

    pub fn walk_next<'a, 'b: 'a>(
        cr: Pin<&'a PageEntry<Level4>>,
        translator: &dyn FrameTranslator<Level4, Page4Kb>,
    ) -> Result<Pin<&'b mut PageTable<Level3>>, FrameError> {
        unsafe { Ok(translator.translate_frame(cr.frame()?)) }
    }
}

pub enum Level3Walk<'a> {
    PageTable(Pin<&'a mut PageTable<Level2>>),
    HugePage(PhysicalFrame<Page1Gb>),
}
impl PageTable<Level3> {
    pub fn walk_next<'a, 'b: 'a>(
        cr: Pin<&'a PageEntry<Level3>>,
        translator: &dyn FrameTranslator<Level3, Page4Kb>,
    ) -> Result<Level3Walk<'b>, FrameError> {
        match cr.frame()? {
            MappedLevel3Page::Page4Kb(frame) => unsafe {
                Ok(Level3Walk::PageTable(translator.translate_frame(frame)))
            },
            MappedLevel3Page::Page1Gb(frame) => Ok(Level3Walk::HugePage(frame)),
        }
    }

    /// # Safety
    ///
    /// The virtual address must have a valid 1Gb page offset
    pub unsafe fn translate_with_frame(
        c: PhysicalFrame<Page1Gb>,
        virt: VirtualAddr,
    ) -> PhysicalAddr {
        c.ptr() + u64::from(virt.page_offset())
    }
}

pub enum Level2Walk<'a> {
    PageTable(Pin<&'a mut PageTable<Level1>>),
    HugePage(PhysicalFrame<Page2Mb>),
}

impl PageTable<Level2> {
    pub fn walk_next<'a, 'b: 'a>(
        cr: Pin<&'a PageEntry<Level2>>,
        translator: &dyn FrameTranslator<Level2, Page4Kb>,
    ) -> Result<Level2Walk<'b>, FrameError> {
        match cr.frame()? {
            MappedLevel2Page::Page4Kb(frame) => unsafe {
                Ok(Level2Walk::PageTable(translator.translate_frame(frame)))
            },
            MappedLevel2Page::Page2Mb(frame) => Ok(Level2Walk::HugePage(frame)),
        }
    }

    /// # Safety
    ///
    /// The virtual address must have a valid 2Mb page offset
    pub unsafe fn translate_with_frame(
        c: PhysicalFrame<Page2Mb>,
        virt: VirtualAddr,
    ) -> PhysicalAddr {
        c.ptr() + u64::from(virt.page_offset())
    }
}

impl PageTable<Level1> {
    pub fn translate_with_index(
        self: Pin<&Self>,
        idx: PageTableIndex<Level1>,
        virt: VirtualAddr,
    ) -> Result<PhysicalAddr, FrameError> {
        self[idx]
            .frame()
            .map(|f| f.ptr() + u64::from(virt.page_offset()))
    }
}

impl<LEVEL: PageLevel> PageTable<LEVEL> {
    pub fn zero(&mut self) {
        self.entries.iter_mut().for_each(PageEntry::clear);
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

impl<'a> Level2Walk<'a> {
    pub fn try_into_table(self) -> Result<Pin<&'a mut PageTable<Level1>>, FrameError> {
        match self {
            Level2Walk::PageTable(table) => Ok(table),
            Level2Walk::HugePage(_) => Err(FrameError::UnexpectedHugePage),
        }
    }
}

impl<'a> Level3Walk<'a> {
    pub fn try_into_table(self) -> Result<Pin<&'a mut PageTable<Level2>>, FrameError> {
        match self {
            Level3Walk::PageTable(table) => Ok(table),
            Level3Walk::HugePage(_) => Err(FrameError::UnexpectedHugePage),
        }
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
    type Prev = ();
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
