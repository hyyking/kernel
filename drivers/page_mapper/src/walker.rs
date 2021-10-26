use core::pin::Pin;

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        entry::{Flags, PageEntry},
        frame::{FrameError, FrameTranslator},
        table::{Level1, Level2, Level2Walk, Level3, Level3Walk, Level4, PageLevel, PageTable},
        NotGiantPageSize, NotHugePageSize, Page4Kb, PageCheck, PageSize,
    },
};

use crate::FrameAllocator;

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

    pub fn try_translate_addr(&mut self, addr: VirtualAddr) -> Result<PhysicalAddr, FrameError> {
        let page = self.level4();

        let entry = page.index_pin_mut(addr.page_table_index(Level4));
        let page = self.walk_level3(entry).map_err(|(_, err)| err)?;

        let entry = page.index_pin_mut(addr.page_table_index(Level3));
        let page = match self.walk_level2(entry).map_err(|(_, err)| err)? {
            Level3Walk::PageTable(table) => table,

            // SAFETY: we hold a valid huge page frame
            Level3Walk::HugePage(frame) => unsafe {
                return Ok(PageTable::<Level3>::translate_with_frame(frame, addr));
            },
        };

        // SAFETY: Level 3 page table must exist since we check for existence in walk_level2
        let entry = page.index_pin_mut(addr.page_table_index(Level2));
        let page = match self.walk_level1(entry).map_err(|(_, err)| err)? {
            Level2Walk::PageTable(table) => table,
            // SAFETY: we hold a valid huge page frame
            Level2Walk::HugePage(frame) => unsafe {
                return Ok(PageTable::<Level2>::translate_with_frame(frame, addr));
            },
        };

        let index = addr.page_table_index(Level1);
        // SAFETY: Level 1 page table must exist since we check for existence in walk_level1
        page.as_ref().translate_with_index(index, addr)
    }

    pub(crate) fn walk_level3<'a>(
        &self,
        entry: Pin<&'a mut PageEntry<Level4>>,
    ) -> Result<Pin<&'a mut PageTable<Level3>>, (Pin<&'a mut PageEntry<Level4>>, FrameError)> {
        PageTable::<Level4>::walk_next(entry.as_ref(), &self.translator).map_err(|err| (entry, err))
    }

    pub(crate) fn walk_level2<'a>(
        &self,
        entry: Pin<&'a mut PageEntry<Level3>>,
    ) -> Result<Level3Walk<'a>, (Pin<&'a mut PageEntry<Level3>>, FrameError)> {
        match PageTable::<Level3>::walk_next(entry.as_ref(), &self.translator) {
            Ok(table) => Ok(table),
            Err(err) => Err((entry, err)),
        }
    }

    pub(crate) fn walk_level1<'a>(
        &self,
        entry: Pin<&'a mut PageEntry<Level2>>,
    ) -> Result<Level2Walk<'a>, (Pin<&'a mut PageEntry<Level2>>, FrameError)> {
        PageTable::<Level2>::walk_next(entry.as_ref(), &self.translator).map_err(|err| (entry, err))
    }

    pub fn level4(&self) -> Pin<&mut PageTable<Level4>> {
        PageTable::new(libx64::control::cr3(), &self.translator)
    }
}

pub trait WalkResultExt<'a, L, const N: u64>
where
    PageCheck<N>: PageSize,
    L: PageLevel,
{
    fn or_create<T, A>(
        self,
        flags: Flags,
        t: &T,
        a: &mut A,
    ) -> Result<Pin<&'a mut PageTable<L>>, FrameError>
    where
        T: FrameTranslator<L::Prev, Page4Kb>,
        A: FrameAllocator<N>;

    fn try_into_table(self) -> Result<Pin<&'a mut PageTable<L>>, FrameError>;
}

impl<'a> WalkResultExt<'a, Level3, Page4Kb>
    for Result<Pin<&'a mut PageTable<Level3>>, (Pin<&'a mut PageEntry<Level4>>, FrameError)>
{
    fn or_create<T, A>(
        self,
        flags: Flags,
        t: &T,
        a: &mut A,
    ) -> Result<Pin<&'a mut PageTable<Level3>>, FrameError>
    where
        T: FrameTranslator<Level4, Page4Kb>,
        A: FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table),
            Err((prev, FrameError::EntryMissing)) => {
                let frame = a.alloc()?;
                trace!("Allocating level3 page table");

                // SAFETY: we just allocated the page so we own it and we are the only one
                // modifying this entry which will be valid.
                unsafe {
                    let prev = prev.get_unchecked_mut();
                    prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                    prev.set_frame(frame);

                    let mut page = t.translate_frame(frame);
                    page.as_mut().get_unchecked_mut().zero();
                    Ok(page)
                }
            }
            Err((_, err)) => Err(err),
        }
    }

    fn try_into_table(self) -> Result<Pin<&'a mut PageTable<Level3>>, FrameError> {
        self.map_err(|(_, err)| err)
    }
}

impl<'a> WalkResultExt<'a, Level2, Page4Kb>
    for Result<Level3Walk<'a>, (Pin<&'a mut PageEntry<Level3>>, FrameError)>
{
    fn or_create<T, A>(
        self,
        flags: Flags,
        t: &T,
        a: &mut A,
    ) -> Result<Pin<&'a mut PageTable<Level2>>, FrameError>
    where
        T: FrameTranslator<Level3, Page4Kb>,
        A: FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table.try_into_table()?),
            Err((prev, FrameError::EntryMissing)) => {
                let frame = a.alloc()?;
                trace!("Allocating level2 page table");

                // SAFETY: we just allocated the page so we own it and we are the only one
                // modifying this entry which will be valid.
                unsafe {
                    let prev = prev.get_unchecked_mut();
                    prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                    prev.set_frame(frame);

                    let mut page = t.translate_frame(frame);
                    page.as_mut().get_unchecked_mut().zero();
                    Ok(page)
                }
            }
            Err((_, err)) => Err(err),
        }
    }

    fn try_into_table(self) -> Result<Pin<&'a mut PageTable<Level2>>, FrameError> {
        self.map_err(|(_, err)| err)
            .and_then(|table| table.try_into_table())
    }
}

impl<'a> WalkResultExt<'a, Level1, Page4Kb>
    for Result<Level2Walk<'a>, (Pin<&'a mut PageEntry<Level2>>, FrameError)>
{
    fn or_create<T, A>(
        self,
        flags: Flags,
        t: &T,
        a: &mut A,
    ) -> Result<Pin<&'a mut PageTable<Level1>>, FrameError>
    where
        T: FrameTranslator<Level2, Page4Kb>,
        A: FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table.try_into_table()?),
            Err((prev, FrameError::EntryMissing)) => {
                let frame = a.alloc()?;
                trace!("Allocating level1 page table");

                // SAFETY: we just allocated the page so we own it and we are the only one
                // modifying this entry which will be valid.
                unsafe {
                    let prev = prev.get_unchecked_mut();
                    prev.set_flags(flags | Flags::PRESENT | Flags::RW);
                    prev.set_frame(frame);

                    let mut page = t.translate_frame(frame);
                    page.as_mut().get_unchecked_mut().zero();
                    Ok(page)
                }
            }
            Err((_, err)) => Err(err),
        }
    }

    fn try_into_table(self) -> Result<Pin<&'a mut PageTable<Level1>>, FrameError> {
        self.map_err(|(_, err)| err)
            .and_then(|table| table.try_into_table())
    }
}
