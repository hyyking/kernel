#![feature(custom_test_frameworks)]
#![feature(asm)]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::infra::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_main]
#![no_std]

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

#[macro_use]
mod infra;
mod init;
pub mod kalloc;
mod pagealloc;

bootloader::entry_point!(kmain);

pub fn kmain(bi: &'static bootloader::BootInfo) -> ! {
    qemu_logger::init().expect("unable to initialize logger");

    kprintln!("[OK] kernel loaded");

    let pmo = VirtualAddr::new(bi.physical_memory_offset);

    init::kinit();
    libx64::sti();

    unsafe {
        use libx64::paging::{entry::Flags, frame::PhysicalFrame, page::Page, Page4Kb};
        use page_mapper::OffsetMapper;

        let mut walker = OffsetMapper::new(pmo);

        dbg!(walker
            .try_translate_addr(VirtualAddr::new(0x201008))
            .unwrap());
        dbg!(walker.try_translate_addr(pmo).unwrap());
        dbg!(walker
            .try_translate_addr(VirtualAddr::new(0xb8000))
            .unwrap());

        use libx64::paging::frame::FrameAllocator;
        let mut alloc = pagealloc::BootInfoFrameAllocator::init(&bi.memory_map);

        let page: Page<Page4Kb> = Page::containing(crate::kalloc::HEAP_OFFSET);
        let frame: PhysicalFrame<Page4Kb> = alloc.alloc().unwrap();

        walker
            .map_4kb_page(
                page,
                frame,
                Flags::PRESENT | Flags::RW | Flags::US,
                &mut alloc,
            )
            .unwrap();
        libx64::paging::invalidate_tlb();

        let test = vec![1u128, 2];
        let test2 = alloc::boxed::Box::new(2u64);
        let test = alloc::boxed::Box::new(3u64);
        drop(test2);
        let test = alloc::boxed::Box::new(2u64);

        debug!("{}", test);
        debug!("{:#?}", &*crate::kalloc::GLOBAL_ALLOC.resource().lock());
        drop(test);

        dbg!(walker.try_translate_addr(page.ptr()).unwrap());
        let page_ptr = page.ptr().as_u64() as *mut u64;
        page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e);
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
