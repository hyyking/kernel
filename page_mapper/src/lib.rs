#![no_std]

pub mod offset;

use core::ptr::NonNull;

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        frame::{FrameError, FrameTranslator},
        table::{Level1, Level2, Level3, Level4, PageTable},
        NotGiantPageSize, NotHugePageSize, Page4Kb, PageCheck, PageSize,
    },
};

pub enum WalkState {
    Page4(NonNull<PageTable<Level4>>),
    Page3(NonNull<PageTable<Level3>>),
    Page2(NonNull<PageTable<Level2>>),
    Page1(NonNull<PageTable<Level1>>),
}

pub struct PageWalker<T, const N: u64>
where
    PageCheck<N>: PageSize,
    T: FrameTranslator<(), Page4Kb>,
    T: FrameTranslator<Level4, Page4Kb>,
    T: FrameTranslator<Level3, Page4Kb>,
    T: FrameTranslator<Level2, Page4Kb>,
{
    translator: T,
    state: WalkState,
}

impl<T, const N: u64> PageWalker<T, N>
where
    PageCheck<N>: NotGiantPageSize + NotHugePageSize,
    T: FrameTranslator<(), Page4Kb>,
    T: FrameTranslator<Level4, Page4Kb>,
    T: FrameTranslator<Level3, Page4Kb>,
    T: FrameTranslator<Level2, Page4Kb>,
{
    pub fn new(translator: T) -> Self {
        let state = WalkState::Page4(Self::level4(&translator));
        Self { translator, state }
    }

    pub unsafe fn translate_addr(&mut self, addr: VirtualAddr) -> Result<PhysicalAddr, FrameError> {
        loop {
            match self.state {
                WalkState::Page4(mut page) => {
                    let entry = &page.as_ref()[addr.page_table_index(Level4)];
                    let table = page.as_mut().walk_next(entry, &self.translator)?;
                    self.state = WalkState::Page3(table);
                }
                WalkState::Page3(mut page) => {
                    let entry = &page.as_ref()[addr.page_table_index(Level3)];
                    if let Some(table) = page.as_mut().walk_next(entry, &self.translator)? {
                        self.state = WalkState::Page2(table);
                    } else {
                        let level = addr.page_table_index(Level3);
                        let addr = page.as_mut().translate_addr(level, addr);
                        self.state = WalkState::Page4(Self::level4(&self.translator));
                        return addr;
                    }
                }
                WalkState::Page2(mut page) => {
                    let entry = &page.as_ref()[addr.page_table_index(Level2)];

                    if let Some(table) = page.as_mut().walk_next(entry, &self.translator)? {
                        self.state = WalkState::Page1(table);
                    } else {
                        let level = addr.page_table_index(Level2);
                        let addr = page.as_mut().translate_addr(level, addr);
                        self.state = WalkState::Page4(Self::level4(&self.translator));
                        return addr;
                    }
                }
                WalkState::Page1(mut page) => {
                    let level = addr.page_table_index(Level1);
                    let addr = page.as_mut().translate_addr(level, addr);
                    self.state = WalkState::Page4(Self::level4(&self.translator));
                    return addr;
                }
            }
        }
    }

    fn level4(translator: &dyn FrameTranslator<(), Page4Kb>) -> NonNull<PageTable<Level4>> {
        PageTable::new(libx64::control::cr3(), translator)
    }
}
