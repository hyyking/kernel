use crate::binary::PageTables;

use page_mapper::OffsetMapper;

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, FrameRange, FrameTranslator, PhysicalFrame},
        page::PageMapper,
        Page1Gb, Page2Mb, Page4Kb, PageCheck, PageSize,
    },
};

#[repr(C)]
pub struct Kernel {
    start: PhysicalAddr,
    size: u64,
}

impl Kernel {
    pub const fn new(start: PhysicalAddr, size: u64) -> Self {
        Self { start, size }
    }

    pub fn bytes(&self) -> &[u8] {
        let ptr = self.start.ptr::<u8>().unwrap();
        unsafe { core::slice::from_raw_parts(ptr.as_ref(), usize::try_from(self.size).unwrap()) }
    }

    pub fn frames(&self) -> FrameRange<Page4Kb> {
        FrameRange::with_size(self.start, self.size)
    }
}

pub struct Ram {
    start: PhysicalAddr,
    end: PhysicalAddr,
}

impl Ram {
    pub const fn new(start: PhysicalAddr, end: PhysicalAddr) -> Self {
        Self { start, end }
    }

    pub const fn frames<const N: usize>(&self) -> FrameRange<N>
    where
        PageCheck<N>: PageSize,
    {
        let start_frame = PhysicalFrame::<N>::containing(self.start);
        let end_frame = PhysicalFrame::<N>::containing(self.end);
        FrameRange::new(start_frame, end_frame)
    }

    pub fn id_map<M, A, const N: usize>(
        &mut self,
        mapper: &mut M,
        alloc: &mut A,
    ) -> Result<(), FrameError>
    where
        PageCheck<N>: PageSize,
        M: PageMapper<N>,
        A: FrameAllocator<Page4Kb>,
    {
        self.frames::<N>().try_for_each(|frame| {
            mapper
                .id_map(frame, Flags::PRESENT | Flags::RW, alloc)
                .map(libx64::paging::page::TlbFlush::flush)
        })
    }
}

pub struct Bootloader<M> {
    mapper: M,
    page_tables: Option<PageTables>,
}

impl<M> Bootloader<M>
where
    M: PageMapper<Page4Kb>
        + PageMapper<Page2Mb>
        + PageMapper<Page1Gb>
        + FrameTranslator<(), Page4Kb>,
{
    pub fn new(mapper: M) -> Self {
        Self {
            mapper,
            page_tables: None,
        }
    }

    pub fn mapper(&mut self) -> &mut M {
        &mut self.mapper
    }

    pub fn page_tables(&mut self) -> Option<PageTables> {
        self.page_tables.take()
    }

    pub fn map_virtual_memory<A>(
        &mut self,
        frame_allocator: &mut A,
        max_phys_addr: PhysicalAddr,
    ) -> Result<&mut Self, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        // identity-map remaining physical memory (first gigabyte is already identity-mapped)
        Ram::new(PhysicalAddr::new(Page1Gb as u64), max_phys_addr)
            .id_map::<M, _, Page2Mb>(&mut self.mapper, frame_allocator)?;
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
}
