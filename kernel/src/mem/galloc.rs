use kcore::{kalloc::slab::fixed::SlabPage, sync::SpinMutex};
use libx64::{
    address::VirtualAddr,
    paging::{
        page::{Page, PageRange},
        Page4Kb,
    },
    units::bits::Kb,
};

use crate::mem::mmo::MemoryMappedObject;

type AllocatorResource = MemoryMappedObject<SpinMutex<SlabPage<1024>>, Page4Kb>;

pub const HEAP_OFFSET: VirtualAddr = VirtualAddr::new(0x4444_4444_0000);

#[global_allocator]
pub static GLOBAL_ALLOC: AllocatorResource = MemoryMappedObject::new(
    SpinMutex::new(SlabPage::from_page(Page::containing(HEAP_OFFSET))),
    PageRange::with_size(HEAP_OFFSET, 4 * Kb),
);
