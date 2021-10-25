#![no_std]
#![feature(never_type)]

#[macro_use]
extern crate log;

pub mod offset;
pub mod walker;

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, PhysicalFrame},
        page::Page,
        table::{Level1, Level2, Level3, Level4},
        Page4Kb,
    },
};

use crate::walker::WalkResultExt;

pub struct OffsetMapper {
    walker: walker::PageWalker<offset::OffsetWalker<Page4Kb>, Page4Kb>,
}

impl OffsetMapper {
    pub fn new(offset: VirtualAddr) -> Self {
        Self {
            walker: walker::PageWalker::new(offset::OffsetWalker::new(offset)),
        }
    }

    pub unsafe fn try_translate_addr(
        &mut self,
        addr: VirtualAddr,
    ) -> Result<PhysicalAddr, FrameError> {
        self.walker.try_translate_addr(addr)
    }

    pub fn map_4kb_page<A: FrameAllocator<Page4Kb>>(
        &mut self,
        page: Page<Page4Kb>,
        frame: PhysicalFrame<Page4Kb>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<(), FrameError> {
        let addr = page.ptr();
        trace!("Mapping page: {:?} -> {:?}", &page, &frame);

        unsafe {
            let level_4 = self.walker.level4().as_mut();
            let translator = self.walker.translator();

            let entry = &mut level_4[addr.page_table_index(Level4)];
            let level_3 = self
                .walker
                .walk_level3(&entry)
                .or_create(entry, flags, translator, allocator)?
                .as_mut();

            let entry = &mut level_3[addr.page_table_index(Level3)];
            let level_2 = self
                .walker
                .walk_level2(&entry)
                .or_create(entry, flags, translator, allocator)?
                .as_mut();

            let entry = &mut level_2[addr.page_table_index(Level2)];
            let level_1 = self
                .walker
                .walk_level1(&entry)
                .or_create(entry, flags, translator, allocator)?
                .as_mut();

            let entry = &mut level_1[addr.page_table_index(Level1)];
            entry.set_flags(flags);
            entry.set_frame(frame);
        }

        Ok(())
    }
}
