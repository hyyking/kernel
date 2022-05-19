use core::ptr::NonNull;

use libx64::{
    address::VirtualAddr,
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, FrameTranslator},
        table::{Level1, Level2, Level2Walk, Level3, Level3Walk, Level4, PageLevel, PageTable},
        NotGiantPageSize, NotHugePageSize, Page4Kb, PageCheck, PageSize, PinEntryMut, PinTableMut,
    },
};

use crate::Translation;

pub(crate) trait WalkResultExt<'a, T, L, const N: usize>
where
    PageCheck<N>: PageSize,
    T: FrameTranslator<L::Prev, Page4Kb>,
    L: PageLevel,
{
    /// If the frame is absent create a new page table with flags using the [`FrameAllocator`]
    fn or_create<A>(self, flags: Flags, a: &mut A) -> Result<PinTableMut<'a, L>, FrameError>
    where
        A: FrameAllocator<N>;

    /// Try to get a page table reference, this must return an error if the page is huge
    fn try_into_table(self) -> Result<PinTableMut<'a, L>, FrameError>;
}

pub(crate) struct WalkError<'a, T, L> {
    translator: &'a T,
    entry: PinEntryMut<'a, L>,
    error: FrameError,
}

impl<'a, T, L> WalkError<'a, T, L> {
    /// Get a reference to the walk error's error.
    pub(crate) const fn into_error(self) -> FrameError {
        self.error
    }
}

pub(crate) struct PageWalker<T, const N: usize>
where
    PageCheck<N>: PageSize,
{
    level4: NonNull<PageTable<Level4>>,
    translator: T,
}

impl<T, const N: usize> PageWalker<T, N>
where
    PageCheck<N>: PageSize,
    T: FrameTranslator<(), Page4Kb>,
{
    pub(crate) fn new(translator: T) -> Self {
        let level4 = PageTable::new(libx64::control::cr3(), &translator);
        unsafe { Self::new_with_level4(translator, level4) }
    }

    pub const fn translator(&self) -> &T {
        &self.translator
    }

    pub(crate) unsafe fn new_with_level4(translator: T, level4: PinTableMut<'_, Level4>) -> Self {
        let level4 = NonNull::from(level4.get_unchecked_mut());
        Self { level4, translator }
    }
}

#[allow(clippy::trait_duplication_in_bounds)] // clippy ??
impl<T, const N: usize> PageWalker<T, N>
where
    PageCheck<N>: NotHugePageSize + NotGiantPageSize,
    T: FrameTranslator<Level4, Page4Kb>
        + FrameTranslator<Level3, Page4Kb>
        + FrameTranslator<Level2, Page4Kb>,
{
    pub(crate) fn level4<'a>(&self) -> PinTableMut<'a, Level4> {
        unsafe { core::pin::Pin::new_unchecked(&mut *self.level4.as_ptr()) }
    }

    pub(crate) fn try_translate_addr(
        &mut self,
        addr: VirtualAddr,
    ) -> Result<Translation, FrameError> {
        let page = self.level4();

        let entry = page.index_pin_mut(addr.page_table_index(Level4));
        let page = self.walk_level3(entry)?;

        let entry = page.index_pin_mut(addr.page_table_index(Level3));
        let page = match self.walk_level2(entry)? {
            Level3Walk::PageTable(table) => table,

            // SAFETY: we hold a valid huge page frame
            Level3Walk::HugePage(frame, flags) => unsafe {
                return Ok(PageTable::<Level3>::translate_with_frame(
                    frame, addr, flags,
                ));
            },
        };

        // SAFETY: Level 3 page table must exist since we check for existence in walk_level2
        let entry = page.index_pin_mut(addr.page_table_index(Level2));
        let page = match self.walk_level1(entry)? {
            Level2Walk::PageTable(table) => table,
            // SAFETY: we hold a valid huge page frame
            Level2Walk::HugePage(frame, flags) => unsafe {
                return Ok(PageTable::<Level2>::translate_with_frame(
                    frame, addr, flags,
                ));
            },
        };

        let index = addr.page_table_index(Level1);
        // SAFETY: Level 1 page table must exist since we check for existence in walk_level1
        page.as_ref().translate_with_index(index, addr)
    }

    pub(crate) fn walk_level3<'a>(
        &'a self,
        entry: PinEntryMut<'a, Level4>,
    ) -> Result<PinTableMut<'a, Level3>, WalkError<'a, T, Level4>> {
        PageTable::<Level4>::walk_next(entry.as_ref(), &self.translator).map_err(|error| {
            WalkError {
                translator: &self.translator,
                entry,
                error,
            }
        })
    }

    pub(crate) fn walk_level2<'a>(
        &'a self,
        entry: PinEntryMut<'a, Level3>,
    ) -> Result<Level3Walk<'a>, WalkError<'a, T, Level3>> {
        PageTable::<Level3>::walk_next(entry.as_ref(), &self.translator).map_err(|error| {
            WalkError {
                translator: &self.translator,
                entry,
                error,
            }
        })
    }

    pub(crate) fn walk_level1<'a>(
        &'a self,
        entry: PinEntryMut<'a, Level2>,
    ) -> Result<Level2Walk<'a>, WalkError<'a, T, Level2>> {
        PageTable::<Level2>::walk_next(entry.as_ref(), &self.translator).map_err(|error| {
            WalkError {
                translator: &self.translator,
                entry,
                error,
            }
        })
    }
}

impl<'a, T> WalkResultExt<'a, T, Level3, Page4Kb>
    for Result<PinTableMut<'a, Level3>, WalkError<'a, T, Level4>>
where
    T: FrameTranslator<Level4, Page4Kb>,
{
    fn or_create<A>(self, flags: Flags, a: &mut A) -> Result<PinTableMut<'a, Level3>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table),
            Err(WalkError {
                translator,
                entry: mut prev,
                error: FrameError::EntryMissing,
            }) => {
                trace!("Allocating level3 page table");
                let frame = a.alloc()?;

                // SAFETY: we just allocated the page so we own it and we are the only one
                // modifying this entry which will be valid.
                unsafe {
                    prev.as_mut().set_flags(flags | Flags::PRESENT | Flags::RW);
                    prev.as_mut().set_frame(frame);

                    let mut page = translator.translate_frame(frame);
                    page.as_mut().zero();
                    Ok(page)
                }
            }
            Err(error) => Err(error.into_error()),
        }
    }

    fn try_into_table(self) -> Result<PinTableMut<'a, Level3>, FrameError> {
        Ok(self?)
    }
}

impl<'a, T> WalkResultExt<'a, T, Level2, Page4Kb>
    for Result<Level3Walk<'a>, WalkError<'a, T, Level3>>
where
    T: FrameTranslator<Level3, Page4Kb>,
{
    fn or_create<A>(self, flags: Flags, a: &mut A) -> Result<PinTableMut<'a, Level2>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table.try_into_table()?),

            Err(WalkError {
                translator,
                entry: mut prev,
                error: FrameError::EntryMissing,
            }) => {
                trace!("Allocating level2 page table");
                let frame = a.alloc()?;

                // SAFETY: we just allocated the page so we own it and we are the only one
                // modifying this entry which will be valid.
                unsafe {
                    prev.as_mut().set_flags(flags | Flags::PRESENT | Flags::RW);
                    prev.as_mut().set_frame(frame);

                    let mut page = translator.translate_frame(frame);
                    page.as_mut().zero();
                    Ok(page)
                }
            }
            Err(error) => Err(error.into_error()),
        }
    }

    fn try_into_table(self) -> Result<PinTableMut<'a, Level2>, FrameError> {
        self.map_err(WalkError::into_error)
            .and_then(Level3Walk::try_into_table)
    }
}

impl<'a, T> WalkResultExt<'a, T, Level1, Page4Kb>
    for Result<Level2Walk<'a>, WalkError<'a, T, Level2>>
where
    T: FrameTranslator<Level2, Page4Kb>,
{
    fn or_create<A>(self, flags: Flags, a: &mut A) -> Result<PinTableMut<'a, Level1>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        match self {
            Ok(table) => Ok(table.try_into_table()?),
            Err(WalkError {
                translator,
                entry: mut prev,
                error: FrameError::EntryMissing,
            }) => {
                trace!("Allocating level1 page table");
                let frame = a.alloc()?;

                // SAFETY: we just allocated the page so we own it and we are the only one
                // modifying this entry which will be valid.
                unsafe {
                    prev.as_mut().set_flags(flags | Flags::PRESENT | Flags::RW);
                    prev.as_mut().set_frame(frame);

                    let mut page = translator.translate_frame(frame);
                    page.as_mut().zero();
                    Ok(page)
                }
            }
            Err(error) => Err(error.into_error()),
        }
    }

    fn try_into_table(self) -> Result<PinTableMut<'a, Level1>, FrameError> {
        self.map_err(WalkError::into_error)
            .and_then(Level2Walk::try_into_table)
    }
}

impl<'a, T, L> From<WalkError<'a, T, L>> for FrameError
where
    L: PageLevel,
    T: FrameTranslator<L, Page4Kb>,
{
    fn from(this: WalkError<'a, T, L>) -> Self {
        this.into_error()
    }
}
