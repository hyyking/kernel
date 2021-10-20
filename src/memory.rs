use core::cell::Cell;
use core::ptr::NonNull;

use libx64::address::{PhysicalAddr, VirtualAddr};
use libx64::paging::PageTableLevel::*;
use libx64::paging::{FrameError, PageTable};

#[derive(Debug, Clone, Copy)]
pub struct L4AlreadyMapped;

pub fn map_l4_at_offset(offset: VirtualAddr) -> Result<NonNull<PageTable>, L4AlreadyMapped> {
    static mut BASE: Cell<Option<NonNull<PageTable>>> = Cell::new(None);
    unsafe {
        match BASE.get() {
            Some(_) => Err(L4AlreadyMapped),
            None => {
                let base_l4 = libx64::control::cr3().frame();
                let new = get_table_ptr(*base_l4, offset);
                BASE.set(Some(new));
                Ok(new)
            }
        }
    }
}

pub fn translate_address(
    addr: VirtualAddr,
    offset: VirtualAddr,
) -> Result<PhysicalAddr, FrameError> {
    let mut frame = libx64::control::cr3().frame();
    let levels = [Level4, Level3, Level2, Level1];

    for level in levels {
        let table = unsafe { get_table_ptr(*frame, offset).as_ref() };
        frame = table[addr.page_table_index(level)].frame()?;
    }
    Ok(PhysicalAddr::new(
        frame.as_u64() + u64::from(addr.page_offset()),
    ))
}

unsafe fn get_table_ptr(table: PhysicalAddr, offset: VirtualAddr) -> NonNull<PageTable> {
    NonNull::new_unchecked((offset.as_u64() + table.as_u64()) as *mut PageTable)
}
