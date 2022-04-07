#![no_std]
#![no_main]
#![feature(step_trait)]

use core::{
    arch::{asm, global_asm},
    panic::PanicInfo,
};

use bootloader::{
    binary::{
        bootloader::{Bootloader, Kernel, KernelError},
        memory::{BiosFrameAllocator, E820MemoryMap},
        CONFIG,
    },
    boot_info::{FrameBufferInfo, PixelFormat},
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
        PhysicalAddr::new(0x400_000),
        &_kernel_size as *const _ as u64,
    );

    bootloader_main(kernel)
}

fn make_framebuffer() -> (PhysicalAddr, FrameBufferInfo) {
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
    (addr, info)
}

fn bootloader_main(kernel: Result<Kernel, KernelError>) -> ! {
    qemu_logger::init().expect("unable to initialize logger");
    log::info!(
        "BIOS boot at {:?}",
        PhysicalAddr::from_ptr(bootloader_main as *const ())
    );
    let kernel = kernel.expect("invalid kernel no booting will be attempted");

    // Extract lower 8 bits
    let memory_map = unsafe {
        E820MemoryMap::from_memory(
            VirtualAddr::new(&_memory_map as *const _ as u64),
            usize::try_from((mmap_ent & 0xff) as u64).unwrap(),
            core::iter::Step::forward(kernel.frames().last().unwrap(), 1),
        )
    };

    // We identity-map all memory, so the offset between physical and virtual addresses is 0
    let bootloader = Bootloader::<OffsetMapper, _, _>::new(
        kernel,
        BiosFrameAllocator::new(memory_map).unwrap(),
        OffsetMapper::new(VirtualAddr::new(0)),
        CONFIG.boot_info_address.map(VirtualAddr::new),
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

    enable_write_protect_bit();

    if CONFIG.map_framebuffer {
        let (start, info) = make_framebuffer();

        bootloader
            .map_framebuffer(
                start,
                info,
                CONFIG.framebuffer_address.map(VirtualAddr::new),
            )
            .unwrap();
    }

    bootloader.detect_rsdp();

    // NOTE: this could be an opt-in, see other methods (ie. id mapping, offset, temporary, recursive level4)
    // right now this is kinda needed as their is no way to use another method else
    bootloader
        .map_physical_memory(VirtualAddr::new(0x10_0000_0000))
        .expect("couldn't map physical memory");

    bootloader.boot()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("[PANIC]: {}", info);
    libx64::diverging_hlt()
}

/* NOTE: My cpu doesn't support EFER.NX see CPUID feature section 4.1.4 Intel manual
#[inline]
fn enable_nxe_bit() {
    use libx64::control::{efer, set_efer, Efer};
    set_efer(efer() | Efer::NXE);
}
*/

#[inline]
fn enable_write_protect_bit() {
    use libx64::control::{cr0, set_cr0, CR0};
    set_cr0(cr0() | CR0::WP);
}
