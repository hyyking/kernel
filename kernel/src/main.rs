#![feature(custom_test_frameworks)]
#![feature(alloc_error_handler)]
#![feature(allocator_api)]
#![feature(abi_x86_interrupt)]
#![feature(step_trait)]
#![feature(array_chunks)]
#![test_runner(crate::infra::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_main]
#![no_std]
#![allow(clippy::cast_possible_truncation, clippy::missing_panics_doc)]

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate qemu_logger;

extern crate alloc;

use core::panic::PanicInfo;

use libx64::{address::VirtualAddr, paging::page::PageTranslator};

use crate::mem::{context::MemoryLayout, pmm::PhysicalMemoryManager};

#[macro_use]
mod infra;
mod init;
pub mod mem;

bootloader::entry_point!(kmain);
pub fn kmain(bi: &'static mut bootloader::BootInfo) -> ! {
    qemu_logger::init().expect("unable to initialize logger");

    info!("kernel loaded");

    init::kinit();
    libx64::sti();

    let pmo = VirtualAddr::new(bi.physical_memory_offset);

    let mut context = crate::mem::context::MemoryContext::new(
        MemoryLayout::init(&bi.memory_regions).expect("memory layout"),
        page_mapper::OffsetMapper::new(pmo),
        PhysicalMemoryManager::init(&bi.memory_regions),
    );


    dbg!(context.layout().usable.len());
    dbg!(context.mapper.try_translate(pmo).unwrap());

    let f = bi.framebuffer.as_mut().unwrap();
    let info = f.info();
    let mut fb = vesa::framebuffer::Framebuffer::new(f.buffer_mut(), info);

    mem::galloc::GLOBAL_ALLOC
        .map(&mut context)
        .expect("unable to map the global allocator");

    fb.draw(&vesa::text::Text::new(
        "Hello World!",
        80,
        100,
    ))
      .unwrap();

    {
        /*
        let sched_alloc = MemoryMappedObject::new(
            SpinMutex::new(SlabPage::from_page(Page::<Page4Kb>::containing(
                VirtualAddr::new(0x1_0000_4000),
            ))),
            // NOTE: ???
            PageRangeInclusive::<Page4Kb>::with_size(VirtualAddr::new(0x1_0000_4000), 4 * Kb),
        );
        sched_alloc.map(&mut context).expect("scheduler allocator");

        dbg!(alloc::boxed::Box::new_in(1u8, &sched_alloc));
        */
    }

    {
        /*
        use scheduler::{Scheduler, Task};
        let mut scheduler = Scheduler::new(sched_alloc.into_resource());
                scheduler.spawn(async {
                    use kcore::futures::stream::StreamExt;

                    while let Some(key) = crate::init::KEYBOARD.lock().next().await {
                        dbg!(key);
                    }
                });
                scheduler.run();
        */
    }

    #[cfg(test)]
    test_main();

    libx64::diverging_hlt();
}

#[panic_handler]
fn ph(info: &PanicInfo) -> ! {
    error!("PANIC => {}", info);
    libx64::diverging_hlt();
}
