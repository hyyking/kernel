#![feature(custom_test_frameworks)]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::infra::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_main]
#![no_std]

use core::panic::PanicInfo;

use libx64::{
    hlt,
    idt::{lidt, InterruptDescriptorTable as Idt, InterruptFrame},
};

#[macro_use]
pub mod drivers;

#[macro_use]
mod infra;

static mut IDT: Idt = Idt::new();

#[no_mangle]
pub extern "C" fn _start() {
    kprintln!("[OK] kernel loaded");

    unsafe {
        kprintln!("[OK] IDT loaded");
        IDT.set_handler(0x03, test_int3);
        lidt(&IDT);
        asm!("int3");
    }

    kprintln!("[OK] int3");

    #[cfg(test)]
    test_main();

    hlt();
}

pub extern "x86-interrupt" fn test_int3(f: InterruptFrame) {
    kprintln!("{:#?}", f)
}

#[panic_handler]
fn ph(info: &PanicInfo) -> ! {
    kprintln!("[PANIC]: {}", info);
    hlt();
}
