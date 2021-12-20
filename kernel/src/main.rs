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

use scheduler::{Scheduler, Task};

#[macro_use]
mod infra;
mod init;
pub mod mem;

bootloader::entry_point!(kmain);

pub fn kmain(bi: &'static bootloader::BootInfo) -> ! {
    qemu_logger::init().expect("unable to initialize logger");

    kprintln!("[OK] kernel loaded");

    init::kinit();
    libx64::sti();

    {
        let pmo = VirtualAddr::new(bi.physical_memory_offset);

        let layout = MemoryLayout::init(&bi.memory_map).expect("memory layout");
        let walker = page_mapper::OffsetMapper::new(pmo);
        let alloc = BootInfoFrameAllocator::init(&bi.memory_map);

        let mut context = crate::mem::context::MemoryContext::new(layout, walker, alloc);

        mem::galloc::GLOBAL_ALLOC
            .map(&mut context.mapper, &mut context.alloc)
            .expect("unable to map the global allocator");

        dbg!("HERE");
        /*
         * let mut scheduler = Scheduler::new();

        scheduler.spawn(async {
            use kcore::futures::stream::StreamExt;

            while let Some(key) = (&mut *crate::init::KEYBOARD.lock()).next().await {
                kprint!("{}", key)
            }
        });
        scheduler.run();
        */
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
