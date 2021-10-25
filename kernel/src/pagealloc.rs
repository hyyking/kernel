use libx64::paging::{
    frame::{FrameAllocator, FrameError, PhysicalFrame},
    Page4Kb,
};

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
#[derive(Debug)]
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl FrameAllocator<Page4Kb> for BootInfoFrameAllocator {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        let frame = self
            .usable_frames()
            .nth(self.next)
            .ok_or(FrameError::Alloc)?;
        self.next += 1;
        Ok(frame)
    }
}

impl BootInfoFrameAllocator {
    pub fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
    fn usable_frames(&self) -> impl Iterator<Item = PhysicalFrame<Page4Kb>> {
        self.memory_map
            .iter()
            .filter(|r| r.region_type == MemoryRegionType::Usable)
            .map(|r| r.range.start_addr()..r.range.end_addr())
            .flat_map(|r| r.step_by(Page4Kb as usize))
            .map(|addr| {
                libx64::paging::frame::PhysicalFrame::containing(
                    libx64::address::PhysicalAddr::new(addr),
                )
            })
    }
}
