#![no_std]

#[macro_use]
extern crate log;

pub mod offset;
pub mod walker;

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, PhysicalFrame},
        page::{Page, PageMapper},
        table::{Level1, Level2, Level3, Level4},
        Page1Gb, Page2Mb, Page4Kb,
    },
};

use crate::{
    offset::OffsetWalker,
    walker::{PageWalker, WalkResultExt},
};

pub struct OffsetMapper {
    walker: PageWalker<OffsetWalker<Page4Kb>, Page4Kb>,
}

impl OffsetMapper {
    pub fn new(offset: VirtualAddr) -> Self {
        Self {
            walker: PageWalker::new(OffsetWalker::new(offset)),
        }
    }

    pub fn try_translate_addr(&mut self, addr: VirtualAddr) -> Result<PhysicalAddr, FrameError> {
        self.walker.try_translate_addr(addr)
    }
}

impl<A> PageMapper<A, Page4Kb> for OffsetMapper
where
    A: FrameAllocator<Page4Kb>,
{
    fn map(
        &mut self,
        page: Page<Page4Kb>,
        frame: PhysicalFrame<Page4Kb>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<(), FrameError> {
        let addr = page.ptr();
        trace!("Mapping page: {:?} -> {:?}", &page, &frame);

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry).or_create(flags, allocator)?;

        let entry = level_3.index_pin_mut(addr.page_table_index(Level3));
        let level_2 = self.walker.walk_level2(entry).or_create(flags, allocator)?;

        let entry = level_2.index_pin_mut(addr.page_table_index(Level2));
        let level_1 = self.walker.walk_level1(entry).or_create(flags, allocator)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_1.index_pin_mut(addr.page_table_index(Level1));
            entry.as_mut().set_flags(flags);
            entry.as_mut().set_frame(frame);
        }

        Ok(())
    }
}

impl<A> PageMapper<A, Page2Mb> for OffsetMapper
where
    A: FrameAllocator<Page4Kb> + FrameAllocator<Page2Mb>,
{
    fn map(
        &mut self,
        page: Page<Page2Mb>,
        frame: PhysicalFrame<Page2Mb>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<(), FrameError> {
        let addr = page.ptr();
        trace!("Mapping page: {:?} -> {:?}", &page, &frame);

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry).or_create(flags, allocator)?;

        let entry = level_3.index_pin_mut(addr.page_table_index(Level3));
        let level_2 = self.walker.walk_level2(entry).or_create(flags, allocator)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_2.index_pin_mut(addr.page_table_index(Level2));
            entry.as_mut().set_flags(flags | Flags::HUGE);
            entry.as_mut().set_frame(frame);
        }

        Ok(())
    }
}

impl<A> PageMapper<A, Page1Gb> for OffsetMapper
where
    A: FrameAllocator<Page4Kb> + FrameAllocator<Page1Gb>,
{
    fn map(
        &mut self,
        page: Page<Page1Gb>,
        frame: PhysicalFrame<Page1Gb>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<(), FrameError> {
        let addr = page.ptr();
        trace!("Mapping page: {:?} -> {:?}", &page, &frame);

        let level_4 = self.walker.level4();

        let entry = level_4.index_pin_mut(addr.page_table_index(Level4));
        let level_3 = self.walker.walk_level3(entry).or_create(flags, allocator)?;

        // SAFETY: we are the sole owner of this page and the entry will be valid
        unsafe {
            let mut entry = level_3.index_pin_mut(addr.page_table_index(Level3));
            entry.as_mut().set_flags(flags | Flags::HUGE);
            entry.as_mut().set_frame(frame);
        }
        Ok(())
    }
}
