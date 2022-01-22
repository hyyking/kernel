use kcore::{kalloc::slab::dynamic::Slab, sync::SpinMutex};
use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        frame::{FrameAllocator, FrameError, FrameRange, PhysicalFrame},
        page::PageRange,
        Page4Kb,
    },
};

use alloc::alloc::{Allocator, Layout};

use bootloader::boot_info::{MemoryRegionKind, MemoryRegions};

/// A [`FrameAllocator`] that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryRegions,
    alloc: SpinMutex<Slab<4096>>,
}

impl FrameAllocator<Page4Kb> for BootInfoFrameAllocator {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        let ptr = self
            .alloc
            .allocate(Layout::new::<[u8; 512]>())
            .map_err(|err| FrameError::Alloc)
            .map(|ptr| PhysicalFrame::containing(PhysicalAddr::from_ptr(ptr.as_ptr() as *mut u8)));
        ptr
    }
}

impl BootInfoFrameAllocator {
    pub fn init(memory_map: &'static MemoryRegions) -> Self {
        let mut iter = memory_map
            .iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .map(|r| {
                FrameRange::<Page4Kb>::new(PhysicalAddr::new(r.start), PhysicalAddr::new(r.end))
            });

        let page = iter.next().unwrap().start().as_u64();

        BootInfoFrameAllocator {
            memory_map,
            alloc: SpinMutex::new(
                Slab::new(PageRange::with_size(VirtualAddr::new(page), 32 * Page4Kb)).unwrap(),
            ),
        }
    }
}
