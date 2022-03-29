use core::{
    arch::asm,
    mem::{self, MaybeUninit},
    slice,
};

use crate::{
    binary::bootloader::{Bootloader, KernelLoad},
    binary::memory::BiosFrameAllocator,
    boot_info::{BootInfo, FrameBuffer, FrameBufferInfo, MemoryRegion, TlsTemplate},
};

use level_4_entries::UsedLevel4Entries;
use parsed_config::CONFIG;

use libx64::address::{PhysicalAddr, VirtualAddr};
use libx64::paging::{
    entry::Flags,
    frame::{FrameAllocator, FrameError, FrameRange, PhysicalFrame},
    page::{Page, PageMapper, PageRange, PageRangeInclusive, TlbFlush, TlbMethod},
    Page2Mb, Page4Kb,
};

use page_mapper::OffsetMapper;

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

/// Loads the kernel ELF executable into memory and switches to it.
///
/// This function is a convenience function that first calls [`set_up_mappings`], then
/// [`create_boot_info`], and finally [`switch_to_kernel`]. The given arguments are passed
/// directly to these functions, so see their docs for more info.
#[cold]
pub fn load_and_switch_to_kernel<M>(
    bootloader: &mut Bootloader<M>,
    mut frame_allocator: BiosFrameAllocator,
    system_info: SystemInfo,
) -> (Mappings, &'static mut BootInfo) {
    let mut mappings = set_up_mappings(
        bootloader,
        &mut frame_allocator,
        system_info.framebuffer_addr,
        system_info.framebuffer_info.byte_len,
    )
    .unwrap();
    let boot_info =
        create_boot_info(bootloader, frame_allocator, &mut mappings, system_info).unwrap();
    (mappings, boot_info)
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
pub fn set_up_mappings<M>(
    bootloader: &mut Bootloader<M>,
    frame_allocator: &mut BiosFrameAllocator,
    framebuffer_addr: PhysicalAddr,
    framebuffer_size: usize,
) -> Result<Mappings, FrameError> {
    let frame_buffer_loc = frame_buffer_location(&mut bootloader.entries);
    let stack_start_addr = kernel_stack_start_location(&mut bootloader.entries);

    let page_tables = bootloader.page_tables().expect("no page tables");

    let kernel_page_table = &mut page_tables.kernel;

    // Enable support for the no-execute bit in page tables.
    // NOTE: My cpu doesn't support EFER.NX see CPUID feature section 4.1.4 Intel manual
    // enable_nxe_bit();

    // Make the kernel respect the write-protection bits even when in ring 0 by default
    enable_write_protect_bit();

    // create a stack
    let stack_start = Page::<Page4Kb>::containing(stack_start_addr);
    let stack_end = {
        let stack_size = CONFIG.kernel_stack_size.unwrap_or(20 * Page4Kb as u64);
        let end_addr = stack_start_addr + stack_size;
        Page::<Page4Kb>::containing(end_addr)
    };
    let stack = PageRange::new(stack_start, stack_end);

    trace!("Mapping Stack at: {:?}", stack);

    kernel_page_table.map_range_alloc(
        stack,
        Flags::PRESENT | Flags::RW,
        frame_allocator,
        TlbMethod::Ignore,
    )?;

    // identity-map context switch function, so that we don't get an immediate pagefault
    // after switching the active page table
    info!(
        "Mapping context switch at {:?}",
        VirtualAddr::from_ptr(context_switch as *const ())
    );
    kernel_page_table
        .id_map(
            PhysicalFrame::<Page4Kb>::containing_ptr(context_switch as *const ()),
            Flags::PRESENT,
            frame_allocator,
        )
        .map(TlbFlush::ignore)?;

    trace!("Mapping GDT");
    gdt::create_and_load(kernel_page_table, frame_allocator).unwrap();

    // map framebuffer
    let framebuffer_virt_addr = if CONFIG.map_framebuffer {
        let framebuffer_phys_range =
            FrameRange::with_size(framebuffer_addr, framebuffer_size as u64);

        info!("Mapping framebuffer at {:?}", framebuffer_phys_range);

        let page_range =
            PageRangeInclusive::<Page4Kb>::with_size(frame_buffer_loc, framebuffer_size as u64);
        let start_addr = page_range.start();

        kernel_page_table.map_range(
            page_range,
            framebuffer_phys_range,
            Flags::PRESENT | Flags::RW,
            frame_allocator,
            TlbMethod::Ignore,
        )?;

        Some(start_addr)
    } else {
        None
    };

    let physical_memory_offset = {
        info!("Mapping physical memory");

        let offset = VirtualAddr::new(0x10_0000_0000);

        let max_phys = frame_allocator.memory_map().max_phys_addr();

        let memory = FrameRange::<Page2Mb>::new_addr(PhysicalAddr::new(0), max_phys);
        kernel_page_table.map_range(
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
        framebuffer: framebuffer_virt_addr,
        physical_memory_offset,
        stack_end,
    })
}

/// Contains the addresses of all memory mappings set up by [`set_up_mappings`].
pub struct Mappings {
    /// Physical Memory Offset
    pub physical_memory_offset: VirtualAddr,
    /// The stack end page of the kernel.
    pub stack_end: Page<Page4Kb>,
    /// The start address of the framebuffer, if any.
    pub framebuffer: Option<VirtualAddr>,
}

/// Allocates and initializes the boot info struct and the memory map.
///
/// The boot info and memory map are mapped to both the kernel and bootloader
/// address space at the same address. This makes it possible to return a Rust
/// reference that is valid in both address spaces. The necessary physical frames
/// are taken from the given `frame_allocator`.
#[cold]
pub fn create_boot_info<M>(
    bootloader: &mut Bootloader<M>,
    mut frame_allocator: BiosFrameAllocator,
    mappings: &mut Mappings,
    system_info: SystemInfo,
) -> Result<&'static mut BootInfo, FrameError> {
    let boot_info_addr = boot_info_location(&mut bootloader.entries);
    let page_tables = bootloader.page_tables().expect("no page tables");

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

            page_tables
                .kernel
                .map(page, frame, flags, &mut frame_allocator)
                .map(TlbFlush::flush)?;

            // we need to be able to access it too
            page_tables
                .bootloader
                .map(page, frame, flags, &mut frame_allocator)
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
    let memory_regions =
        memory::construct_memory_map(frame_allocator.into_memory_map(), memory_regions)
            .expect("unable to construct memory_map");

    info!("Creating bootinfo");

    let tls_template = match bootloader.kernel {
        KernelLoad::Loaded { tls, .. } => tls,
        _ => panic!("kernel not loaded"),
    };

    // create boot info
    let boot_info = boot_info.write(BootInfo {
        version_major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
        version_minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
        version_patch: env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
        pre_release: !env!("CARGO_PKG_VERSION_PRE").is_empty(),
        memory_regions: memory_regions.into(),
        framebuffer: mappings
            .framebuffer
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

/*
/// Switches to the kernel address space and jumps to the kernel entry point.
#[cold]
pub fn switch_to_kernel(
    page_tables: PageTables,
    mappings: Mappings,
    boot_info: &'static mut BootInfo,
) -> ! {
    let PageTables { mut kernel, .. } = page_tables;

    let addresses = Addresses {
        page_table: PhysicalFrame::<Page4Kb>::containing_ptr(kernel.level4().as_ref().get_ref()),
        stack_top: mappings.stack_end.ptr(),
        entry_point: mappings.entry_point,
        boot_info,
    };

    info!(
        "Jumping to kernel entry point at {:?}",
        addresses.entry_point
    );

    unsafe {
        context_switch(addresses);
    }
}
*/

/// Provides access to the page tables of the bootloader and kernel address space.
pub struct PageTables {
    /// Provides access to the page tables of the bootloader address space.
    pub bootloader: OffsetMapper,
    /// Provides access to the page tables of the kernel address space (not active).
    pub kernel: OffsetMapper,
}

/// Performs the actual context switch.
#[cold]
unsafe fn context_switch(addresses: Addresses) -> ! {
    unsafe {
        asm!(
            "mov cr3, {};
             mov rsp, {};
             push 0;
             jmp {}",
            in(reg) addresses.page_table.ptr().as_u64(),
            in(reg) addresses.stack_top.as_u64(),
            in(reg) addresses.entry_point.as_u64(),
            in("rdi") addresses.boot_info as *const _ as usize,
            options(nostack, noreturn)
        );
    }
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

#[inline]
fn frame_buffer_location(used_entries: &mut UsedLevel4Entries) -> VirtualAddr {
    CONFIG
        .framebuffer_address
        .map(VirtualAddr::new)
        .unwrap_or_else(|| used_entries.get_free_address())
}

#[inline]
fn kernel_stack_start_location(used_entries: &mut UsedLevel4Entries) -> VirtualAddr {
    CONFIG
        .kernel_stack_address
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
