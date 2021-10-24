#![no_std]
#![feature(never_type)]

pub mod offset;
pub mod walker;

use libx64::address::{PhysicalAddr, VirtualAddr};
use libx64::paging::entry::Flags;
use libx64::paging::frame::FrameError;
use libx64::paging::page::Page;
use libx64::paging::{frame::PhysicalFrame, PageCheck, PageSize};

pub trait FrameAllocator<const N: u64>
where
    PageCheck<N>: PageSize,
{
    fn alloc(&mut self) -> Result<PhysicalFrame<N>, ()>;
}

use libx64::paging::Page4Kb;

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
        use crate::walker::WalkResultExt;
        use libx64::paging::table::{Level1, Level2, Level3, Level4};

        let addr = page.ptr();

        unsafe {
            let level_4 = self.walker.level4().as_mut();

            let entry = &mut level_4[addr.page_table_index(Level4)];
            let level_3 = self
                .walker
                .walk_level3(&entry)
                .or_create(entry, flags, allocator)?
                .as_mut();

            let entry = &mut level_3[addr.page_table_index(Level3)];
            let level_2 = self
                .walker
                .walk_level2(&entry)
                .or_create(entry, flags, allocator)?
                .as_mut();

            let entry = &mut level_2[addr.page_table_index(Level2)];
            let level_1 = self
                .walker
                .walk_level1(&entry)
                .or_create(entry, flags, allocator)?
                .as_mut();

            let entry = &mut level_1[addr.page_table_index(Level1)];
            entry.set_flags(flags);
            entry.set_frame(frame);
            debug_assert!(entry.is_present());
        }

        Ok(())
    }
}
