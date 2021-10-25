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
    pub fn translator(&self) -> &T {
        &self.translator
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
        let page = self.level4();

        let entry = &page.as_ref()[addr.page_table_index(Level4)];
        let page = self.walk_level3(entry)?;

        let entry = &page.as_ref()[addr.page_table_index(Level3)];
        let page = match self.walk_level2(entry)? {
            Level3Walk::PageTable(table) => table,
            Level3Walk::HugePage(frame) => {
                return Ok(PageTable::<Level3>::translate_with_frame(frame, addr));
            }
        };

        let entry = &page.as_ref()[addr.page_table_index(Level2)];
        let mut page = match self.walk_level1(entry)? {
            Level2Walk::PageTable(table) => table,
            Level2Walk::HugePage(frame) => {
                return Ok(PageTable::<Level2>::translate_with_frame(frame, addr));
            }
        };

        let index = addr.page_table_index(Level1);
        let addr = page.as_mut().translate_with_index(index, addr);
        return addr;
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
    fn or_create<T, A>(
        self,
        prev: &mut PageEntry<L::Prev>,
        flags: Flags,
        t: &T,
        a: &mut A,
    ) -> Result<NonNull<PageTable<L>>, FrameError>
    where
        T: FrameTranslator<L::Prev, Page4Kb>,
        A: crate::FrameAllocator<N>;
}

impl WalkResultExt<Level3, Page4Kb> for Result<NonNull<PageTable<Level3>>, FrameError> {
    fn or_create<T, A>(
        self,
        prev: &mut PageEntry<Level4>,
        flags: Flags,
        t: &T,
        a: &mut A,
    ) -> Result<NonNull<PageTable<Level3>>, FrameError>
    where
        T: FrameTranslator<Level4, Page4Kb>,
        A: crate::FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table),
            Err(FrameError::EntryMissing) => {
                let frame = a.alloc()?;
                trace!("Allocating level3 page table");

                prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                prev.set_frame(frame);
                unsafe {
                    let mut page = t.translate_frame(frame);
                    page.as_mut().zero();
                    Ok(page)
                }
            }
            Err(err) => Err(err),
        }
    }
}

impl WalkResultExt<Level2, Page4Kb> for Result<Level3Walk, FrameError> {
    fn or_create<T, A>(
        self,
        prev: &mut PageEntry<Level3>,
        flags: Flags,
        t: &T,
        a: &mut A,
    ) -> Result<NonNull<PageTable<Level2>>, FrameError>
    where
        T: FrameTranslator<Level3, Page4Kb>,
        A: crate::FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table.try_into_table()?),
            Err(FrameError::EntryMissing) => {
                let frame = a.alloc()?;
                trace!("Allocating level2 page table");

                prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                prev.set_frame(frame);
                unsafe {
                    let mut page = t.translate_frame(frame);
                    page.as_mut().zero();
                    Ok(page)
                }
            }
            Err(err) => Err(err),
        }
    }
}

impl WalkResultExt<Level1, Page4Kb> for Result<Level2Walk, FrameError> {
    fn or_create<T, A>(
        self,
        prev: &mut PageEntry<Level2>,
        flags: Flags,
        t: &T,
        a: &mut A,
    ) -> Result<NonNull<PageTable<Level1>>, FrameError>
    where
        T: FrameTranslator<Level2, Page4Kb>,
        A: crate::FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table.try_into_table()?),
            Err(FrameError::EntryMissing) => {
                let frame = a.alloc()?;
                trace!("Allocating level1 page table");

                prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                prev.set_frame(frame);
                unsafe {
                    let mut page = t.translate_frame(frame);
                    page.as_mut().zero();
                    Ok(page)
                }
            }
            Err(err) => Err(err),
        }
    }
}
