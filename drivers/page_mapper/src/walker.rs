use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, FrameTranslator},
        table::{Level1, Level2, Level2Walk, Level3, Level3Walk, Level4, PageLevel, PageTable},
        NotGiantPageSize, NotHugePageSize, Page4Kb, PageCheck, PageSize, PinEntryMut, PinTableMut,
    },
};

pub trait WalkResultExt<'a, T, L, const N: u64>
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

pub struct WalkError<'a, T, L> {
    translator: &'a T,
    entry: PinEntryMut<'a, L>,
    error: FrameError,
}

impl<'a, T, L> WalkError<'a, T, L> {
    /// Get a reference to the walk error's error.
    pub const fn into_error(self) -> FrameError {
        self.error
    }
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
    pub const fn new(translator: T) -> Self {
        Self { translator }
    }

    pub const fn translator(&self) -> &T {
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
    pub fn level4(&self) -> PinTableMut<'_, Level4> {
        PageTable::new(libx64::control::cr3(), &self.translator)
    }

    pub fn try_translate_addr(&mut self, addr: VirtualAddr) -> Result<PhysicalAddr, FrameError> {
        let page = self.level4();

        let entry = page.index_pin_mut(addr.page_table_index(Level4));
        let page = self.walk_level3(entry)?;

        let entry = page.index_pin_mut(addr.page_table_index(Level3));
        let page = match self.walk_level2(entry)? {
            Level3Walk::PageTable(table) => table,

            // SAFETY: we hold a valid huge page frame
            Level3Walk::HugePage(frame) => unsafe {
                return Ok(PageTable::<Level3>::translate_with_frame(frame, addr));
            },
        };

        // SAFETY: Level 3 page table must exist since we check for existence in walk_level2
        let entry = page.index_pin_mut(addr.page_table_index(Level2));
        let page = match self.walk_level1(entry)? {
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
            .and_then(|table| table.try_into_table())
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
            .and_then(|table| table.try_into_table())
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
