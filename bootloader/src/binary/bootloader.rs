use core::{arch::asm, mem::MaybeUninit, ops::Deref, ptr::addr_of_mut};

use crate::{
    binary::{memory::BootFrameAllocator, FrameBuffer, FrameBufferInfo, UsedLevel4Entries},
    boot_info::MemoryRegion,
    BootInfo,
};

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    control,
    paging::{
        entry::Flags,
        frame::{FrameError, FrameRange, FrameTranslator, IdentityTranslator, PhysicalFrame},
        page::{Page, PageMapper, PageRange, PageTranslator, TlbMethod},
        Page1Gb, Page2Mb, Page4Kb,
    },
};

use xmas_elf::{header, ElfFile};

#[repr(C)]
pub struct Kernel {
    pub start: PhysicalAddr,
    pub size: u64,
    offset: VirtualAddr,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum KernelError {
    MalformedKernel,
    UnsupportedKernelExecutable,
}

impl Kernel {
    pub fn new(start: PhysicalAddr, size: u64) -> Result<Self, KernelError> {
        let mut kernel = Self {
            start,
            size,
            offset: VirtualAddr::null(),
        };
        if !kernel.start.is_aligned(Page4Kb as u64) {
            return Err(KernelError::MalformedKernel);
        }

        let elf_file = kernel.elf_file();

        header::sanity_check(&elf_file).map_err(|_| KernelError::MalformedKernel)?;
        let kernel_offset = match elf_file.header.pt2.type_().as_type() {
            header::Type::Executable => VirtualAddr::new(0),
            header::Type::SharedObject => VirtualAddr::new(0x400_000),

            a @ (header::Type::None
            | header::Type::Relocatable
            | header::Type::Core
            | header::Type::ProcessorSpecific(_)) => {
                error!("Unsupported Kernel Executable {:?}", a);
                return Err(KernelError::UnsupportedKernelExecutable);
            }
        };
        kernel.offset = kernel_offset;
        Ok(kernel)
    }

    pub fn bytes(&self) -> &[u8] {
        let ptr = self.start.ptr::<u8>().unwrap();
        unsafe { core::slice::from_raw_parts(ptr.as_ref(), usize::try_from(self.size).unwrap()) }
    }

    pub const fn offset(&self) -> VirtualAddr {
        self.offset
    }

    pub fn elf_file(&self) -> ElfFile<'_> {
        ElfFile::new(self.bytes()).expect("kernel bytes are an invalid elf file")
    }

    pub const fn frames(&self) -> FrameRange<Page4Kb> {
        FrameRange::with_size(self.start, self.size)
    }

    pub fn entrypoint(&self) -> VirtualAddr {
        self.offset + self.elf_file().header.pt2.entry_point()
    }
}

#[repr(transparent)]
pub struct LoadedKernel(Kernel);
impl Deref for LoadedKernel {
    type Target = Kernel;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct NoStack;
pub struct Stack {
    start: VirtualAddr,
    size: u64,
}

impl Stack {
    const fn start(&self) -> Page<Page4Kb> {
        Page::containing(self.start)
    }
    const fn end(&self) -> Page<Page4Kb> {
        Page::containing(VirtualAddr::new(self.start.as_u64() + self.size))
    }
    const fn pages(&self) -> PageRange<Page4Kb> {
        PageRange::new(self.start(), self.end())
    }
}

pub struct Bootloader<KM, BM, A, K = Kernel, S = NoStack> {
    entries: UsedLevel4Entries,
    kernel: K,
    stack: S,

    bootloader_mapper: BM,
    kernel_mapper: KM,
    allocator: A,

    bootinfo: &'static mut MaybeUninit<BootInfo>,
    memory_map: &'static mut [MaybeUninit<MemoryRegion>],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BootloaderError {
    AllocatorError(FrameError),
    ElfLoader(&'static str),
}

impl From<FrameError> for BootloaderError {
    fn from(err: FrameError) -> Self {
        Self::AllocatorError(err)
    }
}

impl<KM, BM, A, K, S> Bootloader<KM, BM, A, K, S> {
    pub fn page_tables_alloc(&mut self) -> (&mut KM, &mut BM, &mut A) {
        let Self {
            ref mut kernel_mapper,
            ref mut bootloader_mapper,
            ref mut allocator,
            ..
        } = self;
        (kernel_mapper, bootloader_mapper, allocator)
    }
}

impl<KM, BM, A> Bootloader<KM, BM, A, Kernel, NoStack>
where
    A: BootFrameAllocator,
    KM: PageMapper<Page4Kb> + PageMapper<Page2Mb> + PageTranslator,
    BM: PageMapper<Page4Kb> + PageMapper<Page2Mb> + FrameTranslator<(), Page4Kb>,
{
    pub fn new(
        kernel: Kernel,
        mut allocator: A,
        mapper: BM,
        bootinfo_addr: Option<VirtualAddr>,
    ) -> Result<Self, BootloaderError> {
        info!("Elf file loaded at {:?}", kernel.frames());

        let (mut kernel_mapper, mut bootloader_mapper) =
            Self::create_mappers(&mut allocator, mapper);

        crate::binary::gdt::create_and_load(&mut kernel_mapper, &mut allocator)?;

        let mut entries = UsedLevel4Entries::new();
        let bootinfo_addr = bootinfo_addr.unwrap_or_else(|| entries.get_free_address());
        let (bootinfo, memory_map) = crate::binary::create_boot_info(
            bootinfo_addr,
            &mut kernel_mapper,
            &mut bootloader_mapper,
            &mut allocator,
        )?;

        let mut this = Self {
            entries,
            kernel,
            kernel_mapper,
            bootloader_mapper,
            allocator,
            stack: NoStack,
            bootinfo,
            memory_map,
        };

        this.id_map_virtual_memory()?;
        Ok(this)
    }

    fn id_map_virtual_memory(&mut self) -> Result<(), FrameError> {
        // identity-map remaining physical memory (first gigabyte is already identity-mapped)
        let start_frame = PhysicalFrame::<Page2Mb>::containing(PhysicalAddr::new(Page1Gb as u64));
        let end_frame = PhysicalFrame::<Page2Mb>::containing(self.allocator.max_physical_address());
        let ram = FrameRange::new(start_frame, end_frame);

        self.bootloader_mapper.id_map_range(
            ram,
            Flags::PRESENT | Flags::RW,
            &mut self.allocator,
            TlbMethod::Invalidate,
        )?;
        Ok(())
    }

    fn create_mappers(allocator: &mut A, mapper: BM) -> (KM, BM) {
        info!("Creating page tables");

        unsafe {
            let frame = allocator.alloc().expect("no unused frames");
            let mut page = mapper.translate_frame(frame);
            page.as_mut().clear();

            (
                <KM as PageMapper<Page4Kb>>::from_level4(page),
                <BM as PageMapper<Page4Kb>>::from_level4(control::cr3().table(&IdentityTranslator)),
            )
        }
    }
}

impl<KM, BM, A, S> Bootloader<KM, BM, A, Kernel, S>
where
    A: BootFrameAllocator,
    KM: PageMapper<Page4Kb> + PageMapper<Page2Mb> + PageTranslator,
    BM: PageMapper<Page4Kb> + PageMapper<Page2Mb> + FrameTranslator<(), Page4Kb>,
{
    pub fn load_kernel(
        mut self,
    ) -> Result<Bootloader<KM, BM, A, LoadedKernel, S>, BootloaderError> {
        let mut loader = crate::binary::load_kernel::Loader::new(
            &self.kernel,
            &mut self.kernel_mapper,
            &mut self.allocator,
        );

        self.entries
            .set_elf_loaded(&self.kernel.elf_file(), self.kernel.offset);

        let tls = loader
            .load_segments()
            .map_err(|err| BootloaderError::ElfLoader(err))?;

        unsafe {
            addr_of_mut!((*self.bootinfo.as_mut_ptr()).tls_template).write(tls.into());
        }

        info!("Entry point at {:?}", self.kernel.entrypoint());

        Ok(Bootloader {
            kernel: LoadedKernel(self.kernel),
            entries: self.entries,
            bootloader_mapper: self.bootloader_mapper,
            kernel_mapper: self.kernel_mapper,
            allocator: self.allocator,
            stack: self.stack,
            bootinfo: self.bootinfo,
            memory_map: self.memory_map,
        })
    }
}

impl<KM, BM, A, K> Bootloader<KM, BM, A, K, NoStack>
where
    A: BootFrameAllocator,
    KM: PageMapper<Page4Kb>,
{
    pub fn setup_stack(
        mut self,
        start: Option<VirtualAddr>,
        size: Option<u64>,
    ) -> Result<Bootloader<KM, BM, A, K, Stack>, BootloaderError> {
        let start = start.unwrap_or_else(|| self.entries.get_free_address());
        let size = size.unwrap_or(20 * Page4Kb as u64);
        let stack = Stack { start, size };

        trace!("Mapping stack at: {:?}", stack.pages());

        self.kernel_mapper
            .map_range_alloc(
                stack.pages(),
                Flags::PRESENT | Flags::RW,
                &mut self.allocator,
                TlbMethod::Ignore,
            )
            .map_err(|err| BootloaderError::AllocatorError(err))?;

        Ok(Bootloader {
            kernel: self.kernel,
            entries: self.entries,
            bootloader_mapper: self.bootloader_mapper,
            kernel_mapper: self.kernel_mapper,
            allocator: self.allocator,
            stack,
            bootinfo: self.bootinfo,
            memory_map: self.memory_map,
        })
    }
}

impl<KM, BM, A> Bootloader<KM, BM, A, LoadedKernel, Stack>
where
    A: BootFrameAllocator,
    KM: PageMapper<Page4Kb> + PageMapper<Page2Mb> + PageTranslator,
    BM: PageMapper<Page4Kb> + PageMapper<Page2Mb> + FrameTranslator<(), Page4Kb>,
{
    pub fn map_physical_memory(&mut self, offset: VirtualAddr) -> Result<(), BootloaderError> {
        info!("Mapping physical memory");

        let max_phys = self.allocator.max_physical_address();

        let memory = FrameRange::<Page2Mb>::new_addr(PhysicalAddr::new(0), max_phys);
        self.kernel_mapper.map_range(
            memory
                .clone()
                .map(|frame| Page::<Page2Mb>::containing(offset + frame.ptr().as_u64())),
            memory,
            Flags::PRESENT | Flags::RW,
            &mut self.allocator,
            TlbMethod::Ignore,
        )?;

        unsafe {
            addr_of_mut!((*self.bootinfo.as_mut_ptr()).physical_memory_offset)
                .write(offset.as_u64());
        }

        Ok(())
    }

    pub fn map_framebuffer(
        &mut self,
        buffer_start: PhysicalAddr,
        info: FrameBufferInfo,
        location: Option<VirtualAddr>,
    ) -> Result<(), BootloaderError> {
        let frames = FrameRange::<Page4Kb>::with_size(buffer_start, info.byte_len as u64);

        let location = location.unwrap_or_else(|| self.entries.get_free_address());
        let pages = PageRange::<Page4Kb>::with_size(location, info.byte_len as u64);

        info!("Mapping framebuffer at {:?} to {:?}", frames, pages);

        self.kernel_mapper.map_range(
            pages,
            frames,
            Flags::PRESENT | Flags::RW,
            &mut self.allocator,
            TlbMethod::Ignore,
        )?;

        unsafe {
            addr_of_mut!((*self.bootinfo.as_mut_ptr()).framebuffer).write(
                Some(FrameBuffer {
                    buffer_start: location.as_u64(),
                    buffer_byte_len: info.byte_len,
                    info,
                })
                .into(),
            );
        }

        Ok(())
    }

    pub fn detect_rsdp(&mut self) {
        info!("Detecting root system descriptor page table");
        use rsdp::{
            handler::{AcpiHandler, PhysicalMapping},
            Rsdp,
        };

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
                    virtual_start: core::ptr::NonNull::new(physical_address as *mut _).unwrap(),
                    region_length: size,
                    mapped_length: size,
                    handler: Self,
                }
            }

            fn unmap_physical_region<T>(&self, _region: &PhysicalMapping<Self, T>) {}
        }

        unsafe {
            let rsdp = Rsdp::search_for_on_bios(IdentityMapped)
                .ok()
                .map(|mapping| mapping.physical_start as u64);
            addr_of_mut!((*self.bootinfo.as_mut_ptr()).rsdp_addr).write(rsdp.into());
        }
    }

    #[cold]
    pub fn boot(mut self) -> ! {
        // identity-map context switch function, so that we don't get an immediate pagefault
        // after switching the active page table
        info!(
            "Mapping context switch at {:?}",
            VirtualAddr::from_ptr(context_switch as *const ())
        );
        self.kernel_mapper
            .id_map(
                PhysicalFrame::<Page4Kb>::containing_ptr(context_switch as *const ()),
                Flags::PRESENT,
                &mut self.allocator,
            )
            .map(libx64::paging::page::TlbFlush::ignore)
            .expect("unable to map context switch");

        // create memory regions in the boot info
        let memory_regions = self
            .allocator
            .write_memory_map(self.memory_map)
            .expect("unable to write memory map");
        unsafe {
            addr_of_mut!((*self.bootinfo.as_mut_ptr()).memory_regions).write(memory_regions.into());
        }

        // prepare addresses
        let addresses = Addresses {
            page_table: PhysicalFrame::<Page4Kb>::containing_ptr(
                <KM as PageMapper<Page4Kb>>::level4(&mut self.kernel_mapper)
                    .as_ref()
                    .get_ref(),
            ),
            stack_top: self.stack.end().ptr(),
            entry_point: self.kernel.entrypoint(),
            boot_info: unsafe { self.bootinfo.assume_init_mut() },
        };

        info!(
            "Jumping to kernel entry point at {:?}",
            addresses.entry_point
        );

        // yolo. (at least we have a kernel and a stack :^)
        unsafe { context_switch(addresses) }
    }
}

/// Memory addresses required for the context switch.
struct Addresses {
    page_table: PhysicalFrame<Page4Kb>,
    stack_top: VirtualAddr,
    entry_point: VirtualAddr,
    boot_info: &'static mut crate::boot_info::BootInfo,
}

/// Performs the actual context switch.
#[cold]
#[inline(never)]
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
        )
    }
}
