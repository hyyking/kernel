#![feature(custom_test_frameworks)]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::infra::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_main]
#![no_std]

#[macro_use]
extern crate vga;

#[macro_use]
extern crate log;

#[macro_use]
extern crate qemu_logger;

use core::panic::PanicInfo;

use libx64::address::VirtualAddr;

#[macro_use]
mod infra;
mod init;

bootloader::entry_point!(kmain);

pub fn kmain(bi: &'static bootloader::BootInfo) -> ! {
    qemu_logger::init().expect("unable to initialize logger");

    kprintln!("[OK] kernel loaded");

    let pmo = VirtualAddr::new(bi.physical_memory_offset);

    let addr = VirtualAddr::new(0x201008);
    // let addr = pmo;

    unsafe {
        use libx64::paging::table::{Level2, Level3, Level4, PageTable};
        use page_mapper::OffsetWalker;

        let walker = OffsetWalker::new(pmo);
        let mut table = PageTable::new(libx64::control::cr3(), &walker);

        let entry = &table.as_ref()[addr.page_table_index(Level4)];
        let mut table = table.as_mut().walk_next(entry, &walker).expect("level3");

        let entry = &table.as_ref()[addr.page_table_index(Level3)];
        let mut table = table.as_mut().walk_next(entry, &walker).expect("level2");

        let entry = &table.as_ref()[addr.page_table_index(Level2)];
        let mut table = table.as_mut().walk_next(entry, &walker).expect("level1");

        dbg!(table.as_mut().translate_addr(addr));
    }

    init::kinit();
    libx64::sti();

    #[cfg(test)]
    test_main();

    kprintln!("didn't crash");

    libx64::diverging_hlt();
}

#[panic_handler]
fn ph(info: &PanicInfo) -> ! {
    kprintln!("[PANIC]: {}", info);
    error!("PANIC => {}", info);
    libx64::diverging_hlt();
}
