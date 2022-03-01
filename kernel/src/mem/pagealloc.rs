use kcore::{kalloc::slab::dynamic::Slab, sync::SpinMutex};
use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        frame::{FrameAllocator, FrameError, FrameRangeInclusive, PhysicalFrame},
        page::PageRangeInclusive,
        Page4Kb,
    },
};

use alloc::alloc::{Allocator, Layout};

use bootloader::boot_info::{MemoryRegionKind, MemoryRegions};

/// A [`FrameAllocator`] that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    alloc: SpinMutex<Slab<{ Page4Kb as usize }>>,
}

impl FrameAllocator<Page4Kb> for BootInfoFrameAllocator {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        self.alloc
            .allocate(Layout::new::<[u8; 512]>())
            .map_err(|_err| FrameError::Alloc)
            .map(|ptr| PhysicalAddr::from_ptr(ptr.as_ptr() as *mut u8))
            .map(PhysicalFrame::containing)
    }
}

impl BootInfoFrameAllocator {
    pub fn init(memory_map: &'static MemoryRegions) -> Self {
        let mut iter = memory_map
            .iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .map(|r| {
                // TODO: this is wrong it should be a non inclusive range
                FrameRangeInclusive::<Page4Kb>::new_addr(
                    PhysicalAddr::new(r.start),
                    PhysicalAddr::new(r.end),
                )
            });

        let page = iter.next().unwrap().start().as_u64();

        BootInfoFrameAllocator {
            alloc: SpinMutex::new(
                Slab::new(PageRangeInclusive::with_size(
                    VirtualAddr::new(page),
                    32 * Page4Kb,
                ))
                .unwrap(),
            ),
        }
    }
}
