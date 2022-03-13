use kalloc::slab::SlabPage;
use kcore::sync::SpinMutex;

use libx64::{
    address::VirtualAddr,
    paging::{
        page::{Page, PageRangeInclusive},
        Page4Kb,
    },
    units::Kb,
};

use crate::mem::mmo::MemoryMappedObject;

type AllocatorResource = MemoryMappedObject<SpinMutex<SlabPage>, Page4Kb>;

pub const HEAP_OFFSET: VirtualAddr = VirtualAddr::new(0x4444_4444_0000);

#[global_allocator]
pub static GLOBAL_ALLOC: AllocatorResource = MemoryMappedObject::new(
    SpinMutex::new(SlabPage::from_page(Page::containing(HEAP_OFFSET))),
    PageRangeInclusive::with_size(HEAP_OFFSET, 4 * Kb as u64),
);
