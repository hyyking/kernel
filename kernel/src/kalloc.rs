use alloc::alloc::Layout;

use kcore::{kalloc::slab::fixed::SlabPage, resource::Mapped, sync::SpinMutex};
use libx64::{
    address::VirtualAddr,
    paging::{
        page::{Page, PageRange},
        Page4Kb,
    },
    units::bits::Kb,
};

type AllocatorResource = Mapped<SpinMutex<SlabPage<128>>, Page4Kb>;

pub const HEAP_OFFSET: VirtualAddr = VirtualAddr::new(0x4444_4444_0000);

#[global_allocator]
pub static GLOBAL_ALLOC: AllocatorResource = Mapped::new(
    SpinMutex::new(SlabPage::from_page(Page::containing(HEAP_OFFSET))),
    PageRange::with_size(HEAP_OFFSET, 4 * Kb),
);

#[alloc_error_handler]
fn aeh(error: Layout) -> ! {
    kprintln!("[ALLOC]: {:?}", error);
    error!("ALLOC => {:?}", error);
    libx64::diverging_hlt();
}
