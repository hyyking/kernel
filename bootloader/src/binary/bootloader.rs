use crate::binary::{PageTables, TlsTemplate, UsedLevel4Entries};

use page_mapper::OffsetMapper;

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, FrameRange, FrameTranslator, PhysicalFrame},
        page::{PageMapper, TlbMethod},
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

    pub fn frames(&self) -> FrameRange<Page4Kb> {
        FrameRange::with_size(self.start, self.size)
    }
}

pub enum KernelLoad {
    NotLoaded {
        kernel: Option<Kernel>,
        offset: VirtualAddr,
    },
    Loaded {
        kernel: Kernel,
        offset: VirtualAddr,

        entrypoint: VirtualAddr,
        tls: Option<TlsTemplate>,
    },
}

pub struct Bootloader<M> {
    pub entries: UsedLevel4Entries,
    pub kernel: KernelLoad,
    mapper: M,
    page_tables: Option<PageTables>,
    framebuffer: Option<()>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BootloaderError {
    MalformedKernel,
    UnsupportedKernelExecutable,
}

impl<M> Bootloader<M> {
    pub fn page_tables(&mut self) -> Option<&mut PageTables> {
        self.page_tables.as_mut()
    }
}

impl<M> Bootloader<M>
where
    M: PageMapper<Page4Kb>
        + PageMapper<Page2Mb>
        + PageMapper<Page1Gb>
        + FrameTranslator<(), Page4Kb>,
{
    pub fn new(kernel: Kernel, mapper: M) -> Result<Self, BootloaderError> {
        if !kernel.start.is_aligned(Page4Kb as u64) {
            return Err(BootloaderError::MalformedKernel);
        }

        let elf_file = kernel.elf_file();

        let offset = match elf_file.header.pt2.type_().as_type() {
            header::Type::Executable => VirtualAddr::new(0),
            header::Type::SharedObject => VirtualAddr::new(0x400000),

            header::Type::None
            | header::Type::Relocatable
            | header::Type::Core
            | header::Type::ProcessorSpecific(_) => {
                return Err(BootloaderError::UnsupportedKernelExecutable)
            }
        };

        qemu_logger::dbg!(offset);

        header::sanity_check(&elf_file).map_err(|_| BootloaderError::MalformedKernel)?;
        info!("Elf file loaded at {:?}", kernel.frames());

        Ok(Self {
            entries: UsedLevel4Entries::new(),
            kernel: KernelLoad::NotLoaded {
                kernel: Some(kernel),
                offset,
            },
            mapper,
            framebuffer: None,
            page_tables: None,
        })
    }

    pub fn mapper(&mut self) -> &mut M {
        &mut self.mapper
    }

    pub fn id_map_virtual_memory<A>(
        &mut self,
        frame_allocator: &mut A,
        max_phys_addr: PhysicalAddr,
    ) -> Result<&mut Self, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        // identity-map remaining physical memory (first gigabyte is already identity-mapped)
        let start_frame = PhysicalFrame::<Page2Mb>::containing(PhysicalAddr::new(Page1Gb as u64));
        let end_frame = PhysicalFrame::<Page2Mb>::containing(max_phys_addr);
        let ram = FrameRange::new(start_frame, end_frame);

        self.mapper.id_map_range(
            ram,
            Flags::PRESENT | Flags::RW,
            frame_allocator,
            TlbMethod::Invalidate,
        )?;
        Ok(self)
    }

    pub fn create_page_tables<A>(&mut self, frame_allocator: &mut A) -> &mut Self
    where
        A: FrameAllocator<Page4Kb>,
    {
        log::info!("Creating page tables");

        let kernel_page_table = unsafe {
            let frame = frame_allocator.alloc().expect("no unused frames");
            let mut page = self.mapper().translate_frame(frame);
            page.as_mut().clear();
            OffsetMapper::from_p4(page, VirtualAddr::new(0))
        };

        self.page_tables = Some(PageTables {
            // copy the currently active level 4 page table, because it might be read-only
            // create a new page table hierarchy for the kernel
            bootloader: OffsetMapper::new(VirtualAddr::new(0)),
            kernel: kernel_page_table,
        });

        self
    }

    pub fn load_kernel<A>(&mut self, frame_allocator: &mut A) -> Result<&mut Self, &'static str>
    where
        A: FrameAllocator<Page4Kb>,
    {
        let pt = match self.page_tables {
            Some(ref mut pt) => pt,
            None => {
                log::error!("didn't load the kernel as no page table was provided");
                return Err("kernel loading error: missing page tables");
            }
        };
        let (kernel, offset) = match self.kernel {
            KernelLoad::NotLoaded {
                kernel: ref mut a @ Some(_),
                offset,
            } => (a.take().unwrap(), offset),
            _ => {
                log::error!("didn't load the kernel as it was expected to not be loaded");
                return Err("kernel loading error: missing page tables");
            }
        };

        let mut loader = crate::binary::load_kernel::Loader::new(
            &kernel,
            offset,
            &mut pt.kernel,
            frame_allocator,
        );

        self.entries.set_elf_loaded(&kernel.elf_file(), offset);

        let entrypoint = offset + kernel.elf_file().header.pt2.entry_point();
        let tls = loader.load_segments()?;

        info!("Entry point at {:?}", entrypoint);

        self.kernel = KernelLoad::Loaded {
            kernel,
            offset,
            entrypoint,
            tls,
        };

        Ok(self)
    }

    #[cold]
    pub fn boot(
        mut self,
        mappings: crate::binary::Mappings,
        boot_info: &'static mut crate::BootInfo,
    ) -> ! {
        let PageTables { mut kernel, .. } = self.page_tables.take().expect("page tables not set");

        let entry_point = match self.kernel {
            KernelLoad::Loaded { entrypoint, .. } => entrypoint,
            _ => panic!("kernel not loaded"),
        };

        let addresses = crate::binary::Addresses {
            page_table: PhysicalFrame::<Page4Kb>::containing_ptr(
                kernel.level4().as_ref().get_ref(),
            ),
            stack_top: mappings.stack_end.ptr(),
            entry_point,
            boot_info,
        };

        info!(
            "Jumping to kernel entry point at {:?}",
            addresses.entry_point
        );

        unsafe {
            crate::binary::context_switch(addresses);
        }
    }
}
