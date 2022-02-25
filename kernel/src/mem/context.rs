use bootloader::boot_info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use libx64::{
    address::PhysicalAddr,
    paging::{
        frame::{FrameAllocator, FrameRange},
        page::PageMapper,
        Page4Kb,
    },
};

pub struct MemoryContext<M, A> {
    layout: MemoryLayout,
    pub mapper: M,
    pub alloc: A,
}

pub struct MemoryLayout {
    memory_map: &'static MemoryRegions,
    pub usable: FrameRange<Page4Kb>,
}

impl<M, A> MemoryContext<M, A> {
    pub fn new(layout: MemoryLayout, mapper: M, alloc: A) -> Self {
        Self {
            layout,
            mapper,
            alloc,
        }
    }

    /// Get a reference to the memory context's mapper.
    pub fn mapper(&mut self) -> &mut M {
        &mut self.mapper
    }
}

#[derive(Debug)]
pub struct MemoryInitError;

impl MemoryLayout {
    pub fn init(memory_map: &'static MemoryRegions) -> Result<Self, MemoryInitError> {
        let mut iter = memory_map
            .iter()
            .filter(|&r| r.kind == MemoryRegionKind::Usable)
            .map(|r| {
                FrameRange::<Page4Kb>::new_addr(
                    PhysicalAddr::new(r.start),
                    PhysicalAddr::new(r.end),
                )
            });
        let usable = iter.next().ok_or(MemoryInitError)?;
        while let Some((i, range)) = (&mut iter).enumerate().next() {
            error!("[{}] unmapped memory at: {:?}", i, range)
        }

        Ok(Self { usable, memory_map })
    }

    pub fn memory_map(&self) -> *const MemoryRegions {
        self.memory_map
    }
}

impl core::fmt::Debug for MemoryLayout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MemoryLayout")
            .field("memory_map", &"[ ... ]")
            .field("usable", &self.usable)
            .finish()
    }
}
