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
mod memory;

bootloader::entry_point!(kmain);

pub fn kmain(bi: &'static bootloader::BootInfo) -> ! {
    qemu_logger::init().expect("unable to initialize logger");

    kprintln!("[OK] kernel loaded");

    let pmo = VirtualAddr::new(bi.physical_memory_offset);
    let _l4_map = unsafe {
        memory::map_l4_at_offset(pmo)
            .expect("mapping level 4 page")
            .as_mut()
    };
    dbg!(&bi);

    dbg!(memory::translate_address(VirtualAddr::new(0xb8000), pmo).unwrap());

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
