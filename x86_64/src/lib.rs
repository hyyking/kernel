#![feature(asm)]
#![no_std]

pub mod port;

pub fn hlt() -> ! {
    unsafe {
        asm!("hlt", options(noreturn, nostack, nomem));
    }
}
