#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(const_panic)]
#![no_std]

pub mod address;
pub mod gdt;
pub mod idt;
pub mod port;
pub mod tss;

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
