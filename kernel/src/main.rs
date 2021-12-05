#![feature(custom_test_frameworks)]
#![feature(asm)]
#![feature(alloc_error_handler)]
#![feature(allocator_api)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::infra::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_main]
#![no_std]
#![allow(clippy::cast_possible_truncation, clippy::missing_panics_doc)]

#[macro_use]
extern crate vga;
#[macro_use]
extern crate log;
#[macro_use]
extern crate qemu_logger;
#[macro_use]
extern crate alloc;

use core::panic::PanicInfo;

use libx64::address::VirtualAddr;

use crate::mem::{context::MemoryLayout, pagealloc::BootInfoFrameAllocator};

#[macro_use]
mod infra;
mod init;
pub mod mem;

bootloader::entry_point!(kmain);

pub fn kmain(bi: &'static bootloader::BootInfo) -> ! {
    qemu_logger::init().expect("unable to initialize logger");

    kprintln!("[OK] kernel loaded");

    let pmo = VirtualAddr::new(bi.physical_memory_offset);

    init::kinit();
    libx64::sti();

    {
        use page_mapper::OffsetMapper;

        let mut walker = OffsetMapper::new(pmo);

        dbg!(walker
            .try_translate_addr(VirtualAddr::new(0x0020_1008))
            .unwrap());
        dbg!(walker.try_translate_addr(pmo).unwrap());
        dbg!(walker
            .try_translate_addr(VirtualAddr::new(0xb8000))
            .unwrap());

        let layout = MemoryLayout::init(&bi.memory_map).expect("memory layout");
        dbg!(layout);

        let mut alloc = BootInfoFrameAllocator::init(&bi.memory_map);

        mem::galloc::GLOBAL_ALLOC
            .map(&mut walker, &mut alloc)
            .expect("unable to map");

        let test = vec![1_u128];
        dbg!(test);
        let test2 = alloc::boxed::Box::new(2_u64);
        let test = alloc::boxed::Box::new(3_u64);
        debug!("{:#?}", &*mem::galloc::GLOBAL_ALLOC.resource().lock());
        drop(test2);

        debug!("{}", test);
        debug!("{:#?}", &*mem::galloc::GLOBAL_ALLOC.resource().lock());
    }

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
