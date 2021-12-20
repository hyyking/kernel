use alloc::alloc::Layout;

pub mod context;
pub mod galloc;
pub mod pagealloc;

#[alloc_error_handler]
fn alloc_error_handler(error: Layout) -> ! {
    kprintln!("[ALLOC ERROR]: {:?}", error);
    error!("ALLOC ERROR => {:?}", error);
    libx64::diverging_hlt();
}
