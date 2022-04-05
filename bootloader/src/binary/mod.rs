use core::{
    mem::{self, MaybeUninit},
    slice,
};

use crate::binary::memory::BootFrameAllocator;
use crate::{
    binary::bootloader::{Bootloader, LoadedKernel},
    boot_info::{BootInfo, FrameBuffer, FrameBufferInfo, MemoryRegion, TlsTemplate},
};

use level_4_entries::UsedLevel4Entries;
pub use parsed_config::CONFIG;

use libx64::address::{PhysicalAddr, VirtualAddr};
use libx64::paging::{
    entry::Flags,
    frame::{FrameError, FrameRange, PhysicalFrame},
    page::{Page, PageMapper, PageRangeInclusive, TlbFlush, TlbMethod},
    Page2Mb, Page4Kb,
};

mod gdt;

pub mod bootloader;

/// Provides a type to keep track of used entries in a level 4 page table.
pub mod level_4_entries;

/// Implements a loader for the kernel ELF binary.
pub mod load_kernel;

/// E380 Memory region and BIOS FrameAllocator
pub mod memory;

// Contains the parsed configuration table from the kernel's Cargo.toml.
//
// The layout of the file is the following:
//
// ```
// mod parsed_config {
//    pub const CONFIG: Config = Config { … };
// }
// ```
//
// The module file is created by the build script.
include!(concat!(env!("OUT_DIR"), "/bootloader_config.rs"));

/// Required system information that should be queried from the BIOS or UEFI firmware.
#[derive(Debug, Copy, Clone)]
pub struct SystemInfo {
    /// Start address of the pixel-based framebuffer.
    pub framebuffer_addr: PhysicalAddr,
    /// Information about the framebuffer, including layout and pixel format.
    pub framebuffer_info: FrameBufferInfo,
    /// Address of the _Root System Description Pointer_ structure of the ACPI standard.
    pub rsdp_addr: Option<PhysicalAddr>,
}

/// Sets up mappings for a kernel stack and the framebuffer.
///
/// The `kernel_bytes` slice should contain the raw bytes of the kernel ELF executable. The
/// `frame_allocator` argument should be created from the memory map. The `page_tables`
/// argument should point to the bootloader and kernel page tables. The function tries to parse
/// the ELF file and create all specified mappings in the kernel-level page table.
///
/// The `framebuffer_addr` and `framebuffer_size` fields should be set to the start address and
/// byte length the pixel-based framebuffer. These arguments are required because the functions
/// maps this framebuffer in the kernel-level page table, unless the `map_framebuffer` config
/// option is disabled.
///
/// This function reacts to unexpected situations (e.g. invalid kernel ELF file) with a panic, so
/// errors are not recoverable.
pub fn set_up_mappings<KM, BM, A, S>(
    bootloader: &mut Bootloader<KM, BM, A, LoadedKernel, S>,
) -> Result<Mappings, FrameError>
where
    A: BootFrameAllocator,
    KM: PageMapper<Page4Kb> + PageMapper<Page2Mb>,
{
    // let stack_start_addr = kernel_stack_start_location(&mut bootloader.entries);

    let (kernel_mapper, _, frame_allocator) = bootloader.page_tables_alloc();

    // Enable support for the no-execute bit in page tables.
    // NOTE: My cpu doesn't support EFER.NX see CPUID feature section 4.1.4 Intel manual
    // enable_nxe_bit();

    // Make the kernel respect the write-protection bits even when in ring 0 by default
    enable_write_protect_bit();

    let physical_memory_offset = {
        info!("Mapping physical memory");

        let offset = VirtualAddr::new(0x10_0000_0000);

        let max_phys = frame_allocator.max_physical_address();

        let memory = FrameRange::<Page2Mb>::new_addr(PhysicalAddr::new(0), max_phys);
        kernel_mapper.map_range(
            memory
                .clone()
                .map(|frame| Page::<Page2Mb>::containing(offset + frame.ptr().as_u64())),
            memory,
            Flags::PRESENT | Flags::RW,
            frame_allocator,
            TlbMethod::Ignore,
        )?;
        offset
    };

    Ok(Mappings {
        physical_memory_offset,
    })
}

/// Contains the addresses of all memory mappings set up by [`set_up_mappings`].
pub struct Mappings {
    /// Physical Memory Offset
    pub physical_memory_offset: VirtualAddr,
}

/// Allocates and initializes the boot info struct and the memory map.
///
/// The boot info and memory map are mapped to both the kernel and bootloader
/// address space at the same address. This makes it possible to return a Rust
/// reference that is valid in both address spaces. The necessary physical frames
/// are taken from the given `frame_allocator`.
#[cold]
pub fn create_boot_info<KM, BM, A, S>(
    bootloader: &mut Bootloader<KM, BM, A, LoadedKernel, S>,
    mappings: &mut Mappings,
    framebuffer: Option<VirtualAddr>,
    system_info: SystemInfo,
) -> Result<&'static mut BootInfo, FrameError>
where
    A: BootFrameAllocator,
    KM: PageMapper<Page4Kb>,
    BM: PageMapper<Page4Kb>,
{
    let boot_info_addr = boot_info_location(&mut bootloader.entries);
    let (kernel_mapper, bootloader_mapper, frame_allocator) = bootloader.page_tables_alloc();

    info!("Allocating bootinfo");

    // allocate and map space for the boot info
    let (boot_info, memory_regions) = {
        let boot_info_end = boot_info_addr + mem::size_of::<BootInfo>();

        let memory_map_regions_addr =
            boot_info_end.align_up(u64::try_from(mem::align_of::<MemoryRegion>()).unwrap());

        let regions = frame_allocator.len() + 1; // one region might be split into used/unused
        let memory_map_regions_end =
            memory_map_regions_addr + regions * mem::size_of::<MemoryRegion>();

        let start_page = Page::<Page4Kb>::containing(boot_info_addr);
        let end_page = Page::<Page4Kb>::containing(memory_map_regions_end);
        for page in PageRangeInclusive::new(start_page, end_page) {
            let flags = Flags::PRESENT | Flags::RW;

            let frame = frame_allocator
                .alloc()
                .expect("frame allocation for boot info failed");

            kernel_mapper
                .map(page, frame, flags, frame_allocator)
                .map(TlbFlush::flush)?;

            // we need to be able to access it too
            bootloader_mapper
                .map(page, frame, flags, frame_allocator)
                .map(TlbFlush::flush)?
        }

        let boot_info: &'static mut MaybeUninit<BootInfo> =
            unsafe { boot_info_addr.ptr().unwrap().as_mut() };
        let memory_regions: &'static mut [MaybeUninit<MemoryRegion>] = unsafe {
            slice::from_raw_parts_mut(memory_map_regions_addr.ptr().unwrap().as_mut(), regions)
        };

        (boot_info, memory_regions)
    };

    info!("Creating Memory Map");

    // build memory map
    let memory_regions = frame_allocator
        .write_memory_map(memory_regions)
        .expect("unable to construct memory_map");

    info!("Creating bootinfo");

    let tls_template = bootloader.kernel.tls;

    // create boot info
    let boot_info = boot_info.write(BootInfo {
        version_major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
        version_minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
        version_patch: env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
        pre_release: !env!("CARGO_PKG_VERSION_PRE").is_empty(),
        memory_regions: memory_regions.into(),
        framebuffer: framebuffer
            .map(|addr| FrameBuffer {
                buffer_start: addr.as_u64(),
                buffer_byte_len: system_info.framebuffer_info.byte_len,
                info: system_info.framebuffer_info,
            })
            .into(),
        rsdp_addr: system_info.rsdp_addr.map(|addr| addr.as_u64()).into(),
        physical_memory_offset: mappings.physical_memory_offset.as_u64(),
        tls_template: tls_template.into(),
    });

    Ok(boot_info)
}

/// Memory addresses required for the context switch.
struct Addresses {
    page_table: PhysicalFrame<Page4Kb>,
    stack_top: VirtualAddr,
    entry_point: VirtualAddr,
    boot_info: &'static mut crate::boot_info::BootInfo,
}

#[inline]
fn boot_info_location(used_entries: &mut UsedLevel4Entries) -> VirtualAddr {
    CONFIG
        .boot_info_address
        .map(VirtualAddr::new)
        .unwrap_or_else(|| used_entries.get_free_address())
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
