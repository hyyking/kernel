#![no_std]
#![no_main]
#![feature(step_trait)]

use core::{
    arch::{asm, global_asm},
    panic::PanicInfo,
    ptr::NonNull,
};

use bootloader::{
    binary::{
        bootloader::{Bootloader, Kernel},
        memory::{BiosFrameAllocator, E820MemoryMap},
        SystemInfo, CONFIG,
    },
    boot_info::{FrameBuffer, FrameBufferInfo, PixelFormat},
};

use rsdp::{
    handler::{AcpiHandler, PhysicalMapping},
    Rsdp,
};

use page_mapper::OffsetMapper;

use libx64::address::{PhysicalAddr, VirtualAddr};

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
        "mov ax, 0x0;
         mov ss, ax",
        out("ax") _,
    );

    let kernel = Kernel::new(
        PhysicalAddr::new(0x400000),
        &_kernel_size as *const _ as u64,
    );

    // Extract lower 8 bits
    let memory_map = E820MemoryMap::from_memory(
        VirtualAddr::new(&_memory_map as *const _ as u64),
        usize::try_from((mmap_ent & 0xff) as u64).unwrap(),
        core::iter::Step::forward(kernel.frames().last().unwrap(), 1),
    );

    bootloader_main(kernel, memory_map)
}

fn make_framebuffer() -> FrameBuffer {
    let addr = PhysicalAddr::new(unsafe { u64::from(VBEModeInfo_physbaseptr) });

    let framebuffer_size =
        unsafe { usize::from(VBEModeInfo_yresolution) * usize::from(VBEModeInfo_bytesperscanline) };

    let info = unsafe {
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

    FrameBuffer {
        buffer_start: addr.as_u64(),
        buffer_byte_len: framebuffer_size,
        info,
    }
}

fn bootloader_main(kernel: Kernel, memory_map: E820MemoryMap<'static>) -> ! {
    qemu_logger::init().expect("unable to initialize logger");
    log::info!(
        "BIOS boot at {:?}",
        PhysicalAddr::from_ptr(bootloader_main as *const ())
    );

    // We identity-map all memory, so the offset between physical and virtual addresses is 0
    let bootloader = Bootloader::<OffsetMapper, _, _>::new(
        kernel,
        BiosFrameAllocator::new(memory_map).unwrap(),
        OffsetMapper::new(VirtualAddr::new(0)),
    )
    .unwrap();

    let mut bootloader = bootloader
        .load_kernel()
        .unwrap()
        .setup_stack(
            CONFIG.kernel_stack_address.map(VirtualAddr::new),
            CONFIG.kernel_stack_size,
        )
        .unwrap();

    let framebuffer = make_framebuffer();

    let system_info = SystemInfo {
        framebuffer_addr: PhysicalAddr::new(framebuffer.buffer_start),
        framebuffer_info: framebuffer.info,
        rsdp_addr: detect_rsdp(),
    };

    let framebuffer_start = if CONFIG.map_framebuffer {
        let start = bootloader
            .map_framebuffer(
                framebuffer,
                CONFIG.framebuffer_address.map(VirtualAddr::new),
            )
            .unwrap();
        Some(start)
    } else {
        None
    };

    let mut mappings = bootloader::binary::set_up_mappings(&mut bootloader).unwrap();
    let bootinfo = bootloader::binary::create_boot_info(
        &mut bootloader,
        &mut mappings,
        framebuffer_start,
        system_info,
    )
    .unwrap();

    bootloader.boot(bootinfo)
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
    libx64::diverging_hlt()
}
