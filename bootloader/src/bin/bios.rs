#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    convert::TryFrom,
    panic::PanicInfo,
    ptr::NonNull,
    slice,
};

use bootloader::{
    binary::memory::{BiosFrameAllocator, E820MemoryMap},
    binary::{PageTables, SystemInfo},
    boot_info::{FrameBufferInfo, PixelFormat},
};

use rsdp::{
    handler::{AcpiHandler, PhysicalMapping},
    Rsdp,
};

use page_mapper::OffsetMapper;

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameRange, PhysicalFrame},
        page::PageMapper,
        table::PageTable,
        Page1Gb, Page2Mb, Page4Kb,
    },
};

global_asm!(include_str!("../asm/stage_1.s"));
global_asm!(include_str!("../asm/stage_2.s"));
global_asm!(include_str!(concat!(env!("OUT_DIR"), "/vesa_config.s")));
global_asm!(include_str!("../asm/vesa.s"));
global_asm!(include_str!("../asm/e820.s"));
global_asm!(include_str!("../asm/stage_3.s"));

// values defined in `vesa.s`
extern "C" {
    static VBEModeInfo_physbaseptr: u32;
    static VBEModeInfo_bytesperscanline: u16;
    static VBEModeInfo_xresolution: u16;
    static VBEModeInfo_yresolution: u16;
    static VBEModeInfo_bitsperpixel: u8;
    static VBEModeInfo_redfieldposition: u8;
    static VBEModeInfo_greenfieldposition: u8;
    static VBEModeInfo_bluefieldposition: u8;
}

// Symbols defined in `linker.ld`
extern "C" {
    static mmap_ent: usize;
    static _memory_map: usize;
    static _kernel_start_addr: usize;
    static _kernel_end_addr: usize;
    static _kernel_size: usize;
}

#[no_mangle]
pub unsafe extern "C" fn stage_4() -> ! {
    // Set stack segment
    asm!(
        "mov ax, 0x0; mov ss, ax",
        out("ax") _,
    );

    let kernel_start = 0x400000;
    let kernel_size = &_kernel_size as *const _ as u64;
    let memory_map_addr = &_memory_map as *const _ as u64;
    let memory_map_entry_count = (mmap_ent & 0xff) as u64; // Extract lower 8 bits

    bootloader_main(
        PhysicalAddr::new(kernel_start),
        kernel_size,
        VirtualAddr::new(memory_map_addr),
        memory_map_entry_count,
    )
}

fn bootloader_main(
    kernel_start: PhysicalAddr,
    kernel_size: u64,
    memory_map_addr: VirtualAddr,
    memory_map_entry_count: u64,
) -> ! {
    qemu_logger::init().expect("unable to initialize logger");
    log::info!(
        "BIOS boot at {:?}",
        PhysicalAddr::from_ptr(bootloader_main as *const ())
    );

    let kernel_end = PhysicalFrame::<Page4Kb>::containing(kernel_start + kernel_size - 1u64);
    let next_free = PhysicalFrame::<Page4Kb>::containing(kernel_end.ptr() + Page4Kb);

    let memory_map = E820MemoryMap::from_memory(
        memory_map_addr,
        usize::try_from(memory_map_entry_count).unwrap(),
        next_free,
    );

    let max_phys_addr = memory_map.max_phys_addr();
    let mut frame_allocator = BiosFrameAllocator::new(memory_map).unwrap();

    // We identity-map all memory, so the offset between physical and virtual addresses is 0
    let phys_offset = VirtualAddr::new(0);

    let mut bootloader_page_table = OffsetMapper::new(phys_offset);

    // identity-map remaining physical memory (first gigabyte is already identity-mapped)
    let start_frame = PhysicalFrame::<Page2Mb>::containing(PhysicalAddr::new(Page1Gb as u64));
    let end_frame = PhysicalFrame::<Page2Mb>::containing(max_phys_addr);
    for frame in FrameRange::new(start_frame, end_frame) {
        bootloader_page_table
            .id_map(frame, Flags::PRESENT | Flags::RW, &mut frame_allocator)
            .unwrap()
            .flush()
    }

    let framebuffer_addr = PhysicalAddr::new(unsafe { u64::from(VBEModeInfo_physbaseptr) });
    let framebuffer_info = unsafe {
        let framebuffer_size =
            usize::from(VBEModeInfo_yresolution) * usize::from(VBEModeInfo_bytesperscanline);
        let bytes_per_pixel = VBEModeInfo_bitsperpixel / 8;

        let pixel_format = match (
            VBEModeInfo_redfieldposition,
            VBEModeInfo_greenfieldposition,
            VBEModeInfo_bluefieldposition,
        ) {
            (0, 8, 16) => PixelFormat::RGB,
            (16, 8, 0) => PixelFormat::BGR,
            (r, g, b) => panic!("invalid rgb field positions r: {}, g: {}, b: {}", r, g, b),
        };

        FrameBufferInfo {
            byte_len: framebuffer_size.into(),
            horizontal_resolution: VBEModeInfo_xresolution.into(),
            vertical_resolution: VBEModeInfo_yresolution.into(),
            bytes_per_pixel: bytes_per_pixel.into(),
            stride: (VBEModeInfo_bytesperscanline / u16::from(bytes_per_pixel)).into(),
            pixel_format,
        }
    };

    let page_tables = create_page_tables(&mut frame_allocator);

    let kernel = {
        let ptr = kernel_start.as_u64() as *const u8;
        unsafe { slice::from_raw_parts(ptr, usize::try_from(kernel_size).unwrap()) }
    };

    let system_info = SystemInfo {
        framebuffer_addr,
        framebuffer_info,
        rsdp_addr: detect_rsdp(),
    };
    bootloader::binary::load_and_switch_to_kernel(
        kernel,
        frame_allocator,
        page_tables,
        system_info,
    );
}

/// Creates page table abstraction types for both the bootloader and kernel page tables.
fn create_page_tables(frame_allocator: &mut impl FrameAllocator<Page4Kb>) -> PageTables {
    // We identity-mapped all memory, so the offset between physical and virtual addresses is 0
    let phys_offset = VirtualAddr::new(0);

    // copy the currently active level 4 page table, because it might be read-only
    let bootloader_page_table = OffsetMapper::new(phys_offset);

    // create a new page table hierarchy for the kernel
    let (kernel_page_table, kernel_level_4_frame) = {
        let frame = frame_allocator.alloc().expect("no unused frames");
        let addr = phys_offset + frame.ptr().as_u64();
        let kernel = unsafe {
            let ptr = addr.ptr().unwrap().as_mut();
            *ptr = PageTable::new_zero();
            OffsetMapper::from_p4(core::pin::Pin::new_unchecked(ptr), phys_offset)
        };
        (kernel, frame)
    };
    log::info!("Kernel page table at: {:?}", &kernel_level_4_frame);

    PageTables {
        bootloader: bootloader_page_table,
        kernel: kernel_page_table,
        kernel_level_4_frame,
    }
}

fn detect_rsdp() -> Option<PhysicalAddr> {
    #[derive(Clone)]
    struct IdentityMapped;
    impl AcpiHandler for IdentityMapped {
        unsafe fn map_physical_region<T>(
            &self,
            physical_address: usize,
            size: usize,
        ) -> PhysicalMapping<Self, T> {
            PhysicalMapping {
                physical_start: physical_address,
                virtual_start: NonNull::new(physical_address as *mut _).unwrap(),
                region_length: size,
                mapped_length: size,
                handler: Self,
            }
        }

        fn unmap_physical_region<T>(&self, _region: &PhysicalMapping<Self, T>) {}
    }

    unsafe {
        Rsdp::search_for_on_bios(IdentityMapped)
            .ok()
            .map(|mapping| PhysicalAddr::new(mapping.physical_start as u64))
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("[PANIC]: {}", info);
    loop {
        libx64::cli();
        libx64::hlt();
    }
}
