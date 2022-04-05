use core::arch::asm;

use crate::binary::{memory::BootFrameAllocator, Addresses, TlsTemplate, UsedLevel4Entries};

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
}

impl Kernel {
    pub const fn new(start: PhysicalAddr, size: u64) -> Self {
        Self { start, size }
    }

    pub fn bytes(&self) -> &[u8] {
        let ptr = self.start.ptr::<u8>().unwrap();
        unsafe { core::slice::from_raw_parts(ptr.as_ref(), usize::try_from(self.size).unwrap()) }
    }

    pub fn elf_file(&self) -> ElfFile<'_> {
        ElfFile::new(self.bytes()).expect("kernel bytes are an invalid elf file")
    }

    pub const fn frames(&self) -> FrameRange<Page4Kb> {
        FrameRange::with_size(self.start, self.size)
    }
}

pub struct LoadedKernel {
    pub kernel: Kernel,
    pub entrypoint: VirtualAddr,
    pub tls: Option<TlsTemplate>,
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
    pub entries: UsedLevel4Entries,
    pub kernel: K,
    kernel_offset: VirtualAddr,
    stack: S,
    bootloader_mapper: BM,
    kernel_mapper: KM,
    allocator: A,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BootloaderError {
    MalformedKernel,
    UnsupportedKernelExecutable,
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
    pub fn new(kernel: Kernel, mut allocator: A, mapper: BM) -> Result<Self, BootloaderError> {
        if !kernel.start.is_aligned(Page4Kb as u64) {
            return Err(BootloaderError::MalformedKernel);
        }

        let elf_file = kernel.elf_file();

        header::sanity_check(&elf_file).map_err(|_| BootloaderError::MalformedKernel)?;
        let kernel_offset = match elf_file.header.pt2.type_().as_type() {
            header::Type::Executable => VirtualAddr::new(0),
            header::Type::SharedObject => VirtualAddr::new(0x400_000),

            a @ (header::Type::None
            | header::Type::Relocatable
            | header::Type::Core
            | header::Type::ProcessorSpecific(_)) => {
                error!("Unsupported Kernel Executable {:?}", a);
                return Err(BootloaderError::UnsupportedKernelExecutable);
            }
        };

        info!("Elf file loaded at {:?}", kernel.frames());

        let (mut kernel_mapper, bootloader_mapper) = Self::create_mappers(&mut allocator, mapper);

        trace!("Mapping GDT");
        crate::binary::gdt::create_and_load(&mut kernel_mapper, &mut allocator).unwrap();

        let mut this = Self {
            entries: UsedLevel4Entries::new(),
            kernel,
            kernel_offset,
            kernel_mapper,
            bootloader_mapper,
            allocator,
            stack: NoStack,
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
            self.kernel_offset,
            &mut self.kernel_mapper,
            &mut self.allocator,
        );

        self.entries
            .set_elf_loaded(&self.kernel.elf_file(), self.kernel_offset);

        let entrypoint = self.kernel_offset + self.kernel.elf_file().header.pt2.entry_point();
        let tls = loader
            .load_segments()
            .map_err(|err| BootloaderError::ElfLoader(err))?;

        info!("Entry point at {:?}", entrypoint);

        Ok(Bootloader {
            kernel: LoadedKernel {
                kernel: self.kernel,
                entrypoint,
                tls,
            },
            entries: self.entries,
            kernel_offset: self.kernel_offset,
            bootloader_mapper: self.bootloader_mapper,
            kernel_mapper: self.kernel_mapper,
            allocator: self.allocator,
            stack: self.stack,
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
            kernel_offset: self.kernel_offset,
            bootloader_mapper: self.bootloader_mapper,
            kernel_mapper: self.kernel_mapper,
            allocator: self.allocator,
            stack,
        })
    }
}

impl<KM, BM, A> Bootloader<KM, BM, A, LoadedKernel, Stack>
where
    A: BootFrameAllocator,
    KM: PageMapper<Page4Kb> + PageMapper<Page2Mb> + PageTranslator,
    BM: PageMapper<Page4Kb> + PageMapper<Page2Mb> + FrameTranslator<(), Page4Kb>,
{
    pub fn map_framebuffer(
        &mut self,
        framebuffer: crate::binary::FrameBuffer,
        location: Option<VirtualAddr>,
    ) -> Result<VirtualAddr, BootloaderError> {
        let frames = FrameRange::<Page4Kb>::with_size(
            PhysicalAddr::new(framebuffer.buffer_start),
            framebuffer.buffer_byte_len as u64,
        );

        let location = location.unwrap_or_else(|| self.entries.get_free_address());
        let pages = PageRange::<Page4Kb>::with_size(location, framebuffer.buffer_byte_len as u64);
        let page_start = pages.start();

        info!("Mapping framebuffer at {:?} to {:?}", frames, pages);

        self.kernel_mapper.map_range(
            pages,
            frames,
            Flags::PRESENT | Flags::RW,
            &mut self.allocator,
            TlbMethod::Ignore,
        )?;

        Ok(page_start)
    }

    #[cold]
    pub fn boot(mut self, boot_info: &'static mut crate::BootInfo) -> ! {
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

        let addresses = crate::binary::Addresses {
            page_table: PhysicalFrame::<Page4Kb>::containing_ptr(
                <KM as PageMapper<Page4Kb>>::level4(&mut self.kernel_mapper)
                    .as_ref()
                    .get_ref(),
            ),
            stack_top: self.stack.end().ptr(),
            entry_point: self.kernel.entrypoint,
            boot_info,
        };

        info!(
            "Jumping to kernel entry point at {:?}",
            addresses.entry_point
        );

        unsafe { context_switch(addresses) }
    }
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
