use core::ptr::NonNull;

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        entry::{Flags, PageEntry},
        frame::{FrameError, FrameTranslator},
        table::{Level1, Level2, Level2Walk, Level3, Level3Walk, Level4, PageLevel, PageTable},
        NotGiantPageSize, NotHugePageSize, Page4Kb, PageCheck, PageSize,
    },
};

enum WalkState {
    Page4(NonNull<PageTable<Level4>>),
    Page3(NonNull<PageTable<Level3>>),
    Page2(NonNull<PageTable<Level2>>),
    Page1(NonNull<PageTable<Level1>>),
}

pub struct PageWalker<T, const N: u64>
where
    PageCheck<N>: PageSize,
{
    translator: T,
}

impl<T, const N: u64> PageWalker<T, N>
where
    PageCheck<N>: PageSize,
{
    pub unsafe fn next_table<L: PageLevel>(
        &self,
        entry: &PageEntry<L>,
    ) -> NonNull<PageTable<L::Next>>
    where
        T: FrameTranslator<L, Page4Kb>,
    {
        let frame = libx64::paging::frame::PhysicalFrame::<Page4Kb>::containing(entry.address());
        FrameTranslator::<L, Page4Kb>::translate_frame(&self.translator, frame).cast()
    }
}

impl<T, const N: u64> PageWalker<T, N>
where
    PageCheck<N>: NotHugePageSize + NotGiantPageSize,
    T: FrameTranslator<(), Page4Kb>,
    T: FrameTranslator<Level4, Page4Kb>,
    T: FrameTranslator<Level3, Page4Kb>,
    T: FrameTranslator<Level2, Page4Kb>,
{
    pub fn new(translator: T) -> Self {
        Self { translator }
    }

    pub unsafe fn try_translate_addr(
        &mut self,
        addr: VirtualAddr,
    ) -> Result<PhysicalAddr, FrameError> {
        let mut state = WalkState::Page4(self.level4());
        loop {
            match state {
                WalkState::Page4(page) => {
                    let entry = &page.as_ref()[addr.page_table_index(Level4)];
                    state = WalkState::Page3(self.walk_level3(entry)?);
                }
                WalkState::Page3(page) => {
                    let entry = &page.as_ref()[addr.page_table_index(Level3)];
                    let table = match self.walk_level2(entry)? {
                        Level3Walk::PageTable(table) => table,
                        Level3Walk::HugePage(frame) => {
                            return Ok(PageTable::<Level3>::translate_with_frame(frame, addr));
                        }
                    };
                    state = WalkState::Page2(table);
                }
                WalkState::Page2(page) => {
                    let entry = &page.as_ref()[addr.page_table_index(Level2)];
                    let table = match self.walk_level1(entry)? {
                        Level2Walk::PageTable(table) => table,
                        Level2Walk::HugePage(frame) => {
                            return Ok(PageTable::<Level2>::translate_with_frame(frame, addr));
                        }
                    };
                    state = WalkState::Page1(table);
                }
                WalkState::Page1(mut page) => {
                    let index = addr.page_table_index(Level1);
                    let addr = page.as_mut().translate_with_index(index, addr);
                    return addr;
                }
            }
        }
    }

    pub(crate) fn walk_level3(
        &self,
        entry: &PageEntry<Level4>,
    ) -> Result<NonNull<PageTable<Level3>>, FrameError> {
        PageTable::<Level4>::walk_next(entry, &self.translator)
    }

    pub(crate) fn walk_level2(&self, entry: &PageEntry<Level3>) -> Result<Level3Walk, FrameError> {
        PageTable::<Level3>::walk_next(entry, &self.translator)
    }

    pub(crate) fn walk_level1(&self, entry: &PageEntry<Level2>) -> Result<Level2Walk, FrameError> {
        PageTable::<Level2>::walk_next(entry, &self.translator)
    }

    pub fn level4(&self) -> NonNull<PageTable<Level4>> {
        PageTable::new(libx64::control::cr3(), &self.translator)
    }
}

pub trait WalkResultExt<L, const N: u64>
where
    PageCheck<N>: PageSize,
    L: PageLevel,
{
    fn or_create<A: crate::FrameAllocator<N>>(
        self,
        prev: &mut PageEntry<L::Prev>,
        flags: Flags,
        a: &mut A,
    ) -> Result<NonNull<PageTable<L>>, FrameError>;
}

impl WalkResultExt<Level3, Page4Kb> for Result<NonNull<PageTable<Level3>>, FrameError> {
    fn or_create<A: crate::FrameAllocator<Page4Kb>>(
        self,
        prev: &mut PageEntry<Level4>,
        flags: Flags,
        a: &mut A,
    ) -> Result<NonNull<PageTable<Level3>>, FrameError> {
        match self {
            Ok(table) => Ok(table),
            Err(FrameError::EntryMissing) => {
                let frame = a.alloc().expect("allocation");
                prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                prev.set_frame(frame);
                Ok(frame.ptr().ptr::<PageTable<Level3>>().unwrap())
            }
            Err(err) => Err(err),
        }
    }
}

impl WalkResultExt<Level2, Page4Kb> for Result<Level3Walk, FrameError> {
    fn or_create<A: crate::FrameAllocator<Page4Kb>>(
        self,
        prev: &mut PageEntry<Level3>,
        flags: Flags,
        a: &mut A,
    ) -> Result<NonNull<PageTable<Level2>>, FrameError> {
        match self {
            Ok(table) => Ok(table.try_into_table()?),
            Err(FrameError::EntryMissing) => {
                let frame = a.alloc().expect("allocation");
                prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                prev.set_frame(frame);
                Ok(frame.ptr().ptr::<PageTable<Level2>>().unwrap())
            }
            Err(err) => Err(err),
        }
    }
}

impl WalkResultExt<Level1, Page4Kb> for Result<Level2Walk, FrameError> {
    fn or_create<A: crate::FrameAllocator<Page4Kb>>(
        self,
        prev: &mut PageEntry<Level2>,
        flags: Flags,
        a: &mut A,
    ) -> Result<NonNull<PageTable<Level1>>, FrameError> {
        match self {
            Ok(table) => Ok(table.try_into_table()?),
            Err(FrameError::EntryMissing) => {
                let frame = a.alloc().expect("allocation");
                prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                prev.set_frame(frame);
                Ok(frame.ptr().ptr::<PageTable<Level1>>().unwrap())
            }
            Err(err) => Err(err),
        }
    }
}
