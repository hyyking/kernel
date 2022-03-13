use alloc::alloc::Layout;

pub mod context;
pub mod galloc;
pub mod mmo;
pub mod pmm;

#[alloc_error_handler]
fn alloc_error_handler(error: Layout) -> ! {
    error!("ALLOC ERROR => {:?}", error);
    libx64::diverging_hlt();
}
