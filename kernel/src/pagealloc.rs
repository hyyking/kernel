use libx64::paging::{frame::PhysicalFrame, Page4Kb};
use page_mapper::FrameAllocator;

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl FrameAllocator<Page4Kb> for BootInfoFrameAllocator {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page4Kb>, ()> {
        let frame = self.usable_frames().nth(self.next).unwrap();
        self.next += 1;
        Ok(frame)
    }
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
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
            .flat_map(|r| r.step_by(4096))
            .map(|addr| {
                libx64::paging::frame::PhysicalFrame::containing(
                    libx64::address::PhysicalAddr::new(addr),
                )
            })
    }
}
