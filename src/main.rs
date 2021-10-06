#![feature(custom_test_frameworks)]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::infra::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_main]
#![no_std]

#[macro_use]
extern crate kcore;

use core::panic::PanicInfo;

#[macro_use]
pub mod drivers;
#[macro_use]
mod infra;
mod init;

#[no_mangle]
pub extern "C" fn _start() {
    kprintln!("[OK] kernel loaded");

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
    libx64::diverging_hlt();
}
