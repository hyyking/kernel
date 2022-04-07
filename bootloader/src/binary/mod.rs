use core::{
    mem::{self, MaybeUninit},
    ptr::addr_of_mut,
    slice,
};

use crate::{
    binary::memory::BootFrameAllocator,
    boot_info::{BootInfo, FrameBuffer, FrameBufferInfo, MemoryRegion},
};

use level_4_entries::UsedLevel4Entries;

use libx64::{
    address::VirtualAddr,
    paging::{
        entry::Flags,
        frame::FrameError,
        page::{Page, PageMapper, PageRangeInclusive, TlbFlush},
        Page4Kb,
    },
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
pub use parsed_config::CONFIG;

/// Allocates and initializes the boot info struct and the memory map.
///
/// The boot info and memory map are mapped to both the kernel and bootloader
/// address space at the same address. This makes it possible to return a Rust
/// reference that is valid in both address spaces. The necessary physical frames
/// are taken from the given `frame_allocator`.
#[cold]
pub fn create_boot_info<KM, BM, A>(
    boot_info_addr: VirtualAddr,
    kernel_mapper: &mut KM,
    bootloader_mapper: &mut BM,
    frame_allocator: &mut A,
) -> Result<
    (
        &'static mut MaybeUninit<BootInfo>,
        &'static mut [MaybeUninit<MemoryRegion>],
    ),
    FrameError,
>
where
    A: BootFrameAllocator,
    KM: PageMapper<Page4Kb>,
    BM: PageMapper<Page4Kb>,
{
    info!("Allocating bootinfo");

    // allocate and map space for the boot info

    let boot_info_end = boot_info_addr + mem::size_of::<BootInfo>();

    let memory_map_regions_addr =
        boot_info_end.align_up(u64::try_from(mem::align_of::<MemoryRegion>()).unwrap());

    let regions = frame_allocator.len() + 1; // one region might be split into used/unused
    let memory_map_regions_end = memory_map_regions_addr + regions * mem::size_of::<MemoryRegion>();

    let start_page = Page::<Page4Kb>::containing(boot_info_addr);
    let end_page = Page::<Page4Kb>::containing(memory_map_regions_end);
    for page in PageRangeInclusive::new(start_page, end_page) {
        let flags = Flags::PRESENT | Flags::RW;

        let frame = frame_allocator
            .alloc()
            .expect("frame allocation for boot info failed");

        kernel_mapper
            .map(page, frame, flags, frame_allocator)
            .map(TlbFlush::ignore)?;

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

    unsafe {
        // write version information
        addr_of_mut!((*boot_info.as_mut_ptr()).version_major)
            .write(env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
        addr_of_mut!((*boot_info.as_mut_ptr()).version_minor)
            .write(env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
        addr_of_mut!((*boot_info.as_mut_ptr()).version_patch)
            .write(env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
        addr_of_mut!((*boot_info.as_mut_ptr()).pre_release)
            .write(!env!("CARGO_PKG_VERSION_PRE").is_empty());

        // write defaults that could be changed by the bootloader
        addr_of_mut!((*boot_info.as_mut_ptr()).framebuffer).write(None.into());
        addr_of_mut!((*boot_info.as_mut_ptr()).tls_template).write(None.into());
        addr_of_mut!((*boot_info.as_mut_ptr()).rsdp_addr).write(None.into());
    }

    // NOTE: At this point only the memory map, and physical memory offset should not have a sensible default and must be created before boot
    Ok((boot_info, memory_regions))
}
