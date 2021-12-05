use alloc::alloc::Layout;

pub mod context;
pub mod galloc;
pub mod pagealloc;

#[alloc_error_handler]
fn alloc_error_handler(error: Layout) -> ! {
    kprintln!("[ALLOC]: {:?}", error);
    error!("ALLOC => {:?}", error);
    libx64::diverging_hlt();
}
