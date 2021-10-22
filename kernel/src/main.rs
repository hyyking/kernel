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

    unsafe {
        use page_mapper::{offset::OffsetWalker, PageWalker};

        let mut walker = PageWalker::new(OffsetWalker::new(pmo));

        dbg!(walker.translate_addr(addr).unwrap());
        dbg!(walker.translate_addr(pmo).unwrap());
        dbg!(walker.translate_addr(VirtualAddr::new(0xb8000)).unwrap());
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
