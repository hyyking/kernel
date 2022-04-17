#![no_std]

#[macro_use]
extern crate tracing;

pub mod offset;
pub mod walker;

use libx64::{
    address::VirtualAddr,
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, FrameTranslator, PhysicalFrame},
        page::{Page, PageMapper, PageTranslator, TlbFlush},
        table::{Level1, Level2, Level3, Level4, PageLevel, Translation},
        Page1Gb, Page2Mb, Page4Kb, PinTableMut,
    },
};

use crate::{
    offset::OffsetWalker,
    walker::{PageWalker, WalkResultExt},
};

pub struct OffsetMapper {
    offset: VirtualAddr,
    walker: PageWalker<OffsetWalker<Page4Kb>, Page4Kb>,
}

impl OffsetMapper {
    #[must_use]
    pub fn new(offset: VirtualAddr) -> Self {
        Self {
            offset,
            walker: PageWalker::new(OffsetWalker::new(offset)),
        }
    }

    pub const fn offset(&self) -> VirtualAddr {
        self.offset
    }

    pub fn translator(&self) -> &OffsetWalker<Page4Kb> {
        self.walker.translator()
    }

    #[must_use]
    unsafe fn from_p4(level4: PinTableMut<'_, Level4>, offset: VirtualAddr) -> Self {
        Self {
            offset,
            walker: PageWalker::new_with_level4(OffsetWalker::new(offset), level4),
        }
    }
}

impl PageTranslator for OffsetMapper {
    fn try_translate(&mut self, addr: VirtualAddr) -> Result<Translation, FrameError> {
        self.walker.try_translate_addr(addr)
    }
}

impl PageMapper<Page4Kb> for OffsetMapper {
    unsafe fn from_level4(page: PinTableMut<'_, Level4>) -> Self {
        Self::from_p4(page, VirtualAddr::new(0))
    }

    fn level4(&mut self) -> PinTableMut<'_, Level4> {
        self.walker.level4()
    }

    #[must_use]
    fn map<A>(
        &mut self,
        page: Page<Page4Kb>,
        frame: PhysicalFrame<Page4Kb>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<TlbFlush<Page4Kb>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        let addr = page.ptr();
        let pflags = Flags::PRESENT | Flags::RW | Flags::US;
        trace!("Mapping page: {:?} -> {:?}", &page, &frame);

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self
            .walker
            .walk_level3(entry)
            .or_create(pflags, allocator)?;

        let entry = level_3.index_pin_mut(addr.page_table_index(Level3));
        let level_2 = self
            .walker
            .walk_level2(entry)
            .or_create(pflags, allocator)?;

        let entry = level_2.index_pin_mut(addr.page_table_index(Level2));
        let level_1 = self
            .walker
            .walk_level1(entry)
            .or_create(pflags, allocator)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_1.index_pin_mut(addr.page_table_index(Level1));
            entry.as_mut().set_flags(flags);
            entry.as_mut().set_frame(frame);
        }

        Ok(TlbFlush::new(page))
    }

    fn update_flags(
        &mut self,
        page: Page<Page4Kb>,
        flags: Flags,
    ) -> Result<TlbFlush<Page4Kb>, FrameError> {
        let addr = page.ptr();

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry)?;

        let entry = level_3.index_pin_mut(addr.page_table_index(Level3));
        let level_2 = self.walker.walk_level2(entry)?;

        let entry = level_2
            .try_into_table()?
            .index_pin_mut(addr.page_table_index(Level2));
        let level_1 = self.walker.walk_level1(entry)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_1
                .try_into_table()?
                .index_pin_mut(addr.page_table_index(Level1));
            entry.as_mut().set_flags(flags);
        }

        Ok(TlbFlush::new(page))
    }

    fn unmap(&mut self, page: Page<Page4Kb>) -> Result<TlbFlush<Page4Kb>, FrameError> {
        let addr = page.ptr();

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry)?;

        let entry = level_3.index_pin_mut(addr.page_table_index(Level3));
        let level_2 = self.walker.walk_level2(entry)?;

        let entry = level_2
            .try_into_table()?
            .index_pin_mut(addr.page_table_index(Level2));
        let level_1 = self.walker.walk_level1(entry)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_1
                .try_into_table()?
                .index_pin_mut(addr.page_table_index(Level1));
            entry.as_mut().clear();
        }

        Ok(TlbFlush::new(page))
    }
}

impl PageMapper<Page2Mb> for OffsetMapper {
    unsafe fn from_level4(page: PinTableMut<'_, Level4>) -> Self {
        Self::from_p4(page, VirtualAddr::new(0))
    }

    fn level4(&mut self) -> PinTableMut<'_, Level4> {
        self.walker.level4()
    }

    #[must_use]
    fn map<A>(
        &mut self,
        page: Page<Page2Mb>,
        frame: PhysicalFrame<Page2Mb>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<TlbFlush<Page2Mb>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        let addr = page.ptr();
        let pflags = Flags::PRESENT | Flags::RW | Flags::US;

        trace!("Mapping page: {:?} -> {:?}", &page, &frame);

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self
            .walker
            .walk_level3(entry)
            .or_create(pflags, allocator)?;

        let entry = level_3.index_pin_mut(addr.page_table_index(Level3));
        let level_2 = self
            .walker
            .walk_level2(entry)
            .or_create(pflags, allocator)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_2.index_pin_mut(addr.page_table_index(Level2));
            entry.as_mut().set_flags(flags | Flags::HUGE);
            entry.as_mut().set_frame(frame);
        }

        Ok(TlbFlush::new(page))
    }

    fn update_flags(
        &mut self,
        page: Page<Page2Mb>,
        flags: Flags,
    ) -> Result<TlbFlush<Page2Mb>, FrameError> {
        let addr = page.ptr();

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry)?;

        let entry = level_3.index_pin_mut(addr.page_table_index(Level3));
        let level_2 = self.walker.walk_level2(entry)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_2
                .try_into_table()?
                .index_pin_mut(addr.page_table_index(Level2));
            entry.as_mut().set_flags(flags | Flags::HUGE);
        }

        Ok(TlbFlush::new(page))
    }

    fn unmap(&mut self, page: Page<Page2Mb>) -> Result<TlbFlush<Page2Mb>, FrameError> {
        let addr = page.ptr();

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry)?;

        let entry = level_3.index_pin_mut(addr.page_table_index(Level3));
        let level_2 = self.walker.walk_level2(entry)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_2
                .try_into_table()?
                .index_pin_mut(addr.page_table_index(Level2));
            entry.as_mut().clear();
        }

        Ok(TlbFlush::new(page))
    }
}

impl PageMapper<Page1Gb> for OffsetMapper {
    unsafe fn from_level4(page: PinTableMut<'_, Level4>) -> Self {
        Self::from_p4(page, VirtualAddr::new(0))
    }

    fn level4(&mut self) -> PinTableMut<'_, Level4> {
        self.walker.level4()
    }

    #[must_use]
    fn map<A>(
        &mut self,
        page: Page<Page1Gb>,
        frame: PhysicalFrame<Page1Gb>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<TlbFlush<Page1Gb>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        let addr = page.ptr();
        let pflags = Flags::PRESENT | Flags::RW | Flags::US;
        trace!("Mapping page: {:?} -> {:?}", &page, &frame);

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self
            .walker
            .walk_level3(entry)
            .or_create(pflags, allocator)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_3.index_pin_mut(addr.page_table_index(Level3));
            entry.as_mut().set_flags(flags | Flags::HUGE);
            entry.as_mut().set_frame(frame);
        }
        Ok(TlbFlush::new(page))
    }

    fn update_flags(
        &mut self,
        page: Page<Page1Gb>,
        flags: Flags,
    ) -> Result<TlbFlush<Page1Gb>, FrameError> {
        let addr = page.ptr();

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_3.index_pin_mut(addr.page_table_index(Level3));
            entry.as_mut().set_flags(flags | Flags::HUGE);
        }
        Ok(TlbFlush::new(page))
    }

    fn unmap(&mut self, page: Page<Page1Gb>) -> Result<TlbFlush<Page1Gb>, FrameError> {
        let addr = page.ptr();

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_3.index_pin_mut(addr.page_table_index(Level3));
            entry.as_mut().clear();
        }
        Ok(TlbFlush::new(page))
    }
}

impl FrameTranslator<(), Page4Kb> for OffsetMapper {
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<Page4Kb>,
    ) -> PinTableMut<'a, <() as PageLevel>::Next> {
        FrameTranslator::<(), Page4Kb>::translate_frame(self.translator(), frame)
    }
}
