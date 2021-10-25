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

    pub fn map_4kb_page<A: FrameAllocator<Page4Kb>>(
        &mut self,
        page: Page<Page4Kb>,
        frame: PhysicalFrame<Page4Kb>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<(), FrameError> {
        let addr = page.ptr();
        trace!("Mapping page: {:?} -> {:?}", &page, &frame);

        // SAFETY: Level 4 page table must exist and be valid since we are in long mode
        let level_4 = unsafe { self.walker.level4().as_mut() };
        let translator = self.walker.translator();

        let entry = &mut level_4[addr.page_table_index(Level4)];
        // SAFETY: Level 3 page table will be created if it is missing
        let level_3 = unsafe {
            self.walker
                .walk_level3(entry)
                .or_create(entry, flags, translator, allocator)?
                .as_mut()
        };

        let entry = &mut level_3[addr.page_table_index(Level3)];
        // SAFETY: Level 2 page table will be created if it is missing
        let level_2 = unsafe {
            self.walker
                .walk_level2(entry)
                .or_create(entry, flags, translator, allocator)?
                .as_mut()
        };

        let entry = &mut level_2[addr.page_table_index(Level2)];
        // SAFETY: Level 1 page table will be created if it is missing
        let level_1 = unsafe {
            self.walker
                .walk_level1(entry)
                .or_create(entry, flags, translator, allocator)?
                .as_mut()
        };

        let entry = &mut level_1[addr.page_table_index(Level1)];
        entry.set_flags(flags);
        entry.set_frame(frame);

        Ok(())
    }
}
