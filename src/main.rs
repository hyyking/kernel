#![feature(custom_test_frameworks)]
#![feature(asm)]
#![test_runner(crate::infra::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_main]
#![no_std]

use core::panic::PanicInfo;

use x86_64::hlt;

#[macro_use]
pub mod drivers;

#[macro_use]
mod infra;

pub mod ptr;
pub mod sync;

#[no_mangle]
pub extern "C" fn _start() {
    kprintln!("kernel init");

    qprintln!("HERERERERE");

    #[cfg(test)]
    test_main();

    hlt();
}

#[panic_handler]
fn ph(info: &PanicInfo) -> ! {
    kprintln!("panic: {}", info);
    hlt();
}
