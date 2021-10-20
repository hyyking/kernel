#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![no_std]

pub mod address;
pub mod control;
pub mod descriptors;
pub mod gdt;
pub mod idt;
pub mod paging;
pub mod port;
pub mod rflags;
pub mod segments;

#[repr(u8)]
pub enum Privilege {
    Ring0 = 0b00,
    Ring1 = 0b01,
    Ring2 = 0b10,
    Ring3 = 0b11,
}

impl From<Privilege> for u16 {
    fn from(p: Privilege) -> Self {
        u16::from(p as u8)
    }
}

#[inline]
pub fn hlt() {
    unsafe {
        asm!("hlt", options(nostack, nomem, preserves_flags));
    }
}

#[inline]
pub fn diverging_hlt() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nostack, nomem, preserves_flags));
        }
    }
}

#[inline]
pub fn cli() {
    unsafe {
        asm!("cli", options(nostack, nomem));
    }
}

#[inline]
pub fn sti() {
    unsafe {
        asm!("sti", options(nostack, nomem));
    }
}

pub fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    use rflags::{rflags, RFlags};

    let prev = rflags().contains(RFlags::INTERRUPT_FLAG);
    if prev {
        cli();
    }
    let r = f();
    if prev {
        sti();
    }
    r
}
