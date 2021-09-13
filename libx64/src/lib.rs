#![feature(asm, abi_x86_interrupt)]
#![no_std]

pub mod address;
pub mod idt;
pub mod port;

pub fn hlt() -> ! {
    unsafe {
        asm!("hlt", options(noreturn, nostack, nomem));
    }
}

//  pub fn lidt() {
//      unsafe {
//          asm!("lidt", options(nostack, nomem));
//      }
//      todo!();
//  }
