use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
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
    memory_map: &'static MemoryMap,
    pub low: FrameRange<Page4Kb>,
    pub high: FrameRange<Page4Kb>,
}

impl<M, A> MemoryContext<M, A> {
    pub fn new(layout: MemoryLayout, mapper: M, alloc: A) -> Self {
        Self {
            layout,
            mapper,
            alloc,
        }
    }
}

#[derive(Debug)]
pub struct MemoryInitError;

impl MemoryLayout {
    pub fn init(memory_map: &'static MemoryMap) -> Result<Self, MemoryInitError> {
        let mut iter = memory_map
            .iter()
            .filter(|r| r.region_type == MemoryRegionType::Usable)
            .map(|r| {
                FrameRange::<Page4Kb>::new(
                    PhysicalAddr::new(r.range.start_addr()),
                    PhysicalAddr::new(r.range.end_addr()),
                )
            });
        let low = iter.next().ok_or(MemoryInitError)?;
        let high = iter.next().ok_or(MemoryInitError)?;
        while let Some((i, range)) = (&mut iter).enumerate().next() {
            error!("[{}] unmapped memory at: {:?}", i, range)
        }

        Ok(Self {
            low,
            high,
            memory_map,
        })
    }

    pub fn memory_map(&self) -> *const MemoryMap {
        self.memory_map
    }
}

impl core::fmt::Debug for MemoryLayout {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MemoryLayout")
            .field("memory_map", &"[ ... ]")
            .field("low", &self.low)
            .field("high", &self.high)
            .finish()
    }
}
