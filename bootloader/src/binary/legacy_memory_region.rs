use crate::binary::memory_descriptor::E820MemoryRegion;
use crate::boot_info::{MemoryRegion, MemoryRegionKind};

use core::mem::MaybeUninit;

use libx64::{
    address::PhysicalAddr,
    paging::{
        frame::{FrameAllocator, FrameError, PhysicalFrame},
        Page4Kb,
    },
};

/// A physical frame allocator based on a BIOS or UEFI provided memory map.
pub struct LegacyFrameAllocator {
    /// E820MemoryMap
    pub memory_map: &'static [E820MemoryRegion],
    idx: usize,
    current_descriptor: Option<E820MemoryRegion>,
    next_frame: PhysicalFrame<Page4Kb>,
}

impl LegacyFrameAllocator {
    /// Creates a new frame allocator based on the given legacy memory regions.
    ///
    /// Skips the frame at physical address zero to avoid potential problems. For example
    /// identity-mapping the frame at address zero is not valid in Rust, because Rust's `core`
    /// library assumes that references can never point to virtual address `0`.  
    pub fn new(memory_map: &'static [E820MemoryRegion]) -> Self {
        // skip frame 0 because the rust core library does not see 0 as a valid address
        let start_frame = PhysicalFrame::<Page4Kb>::containing(PhysicalAddr::new(0x1000));

        Self::new_starting_at(start_frame, memory_map)
    }

    /// Creates a new frame allocator based on the given legacy memory regions. Skips any frames
    /// before the given `frame`.
    pub fn new_starting_at(
        frame: PhysicalFrame<Page4Kb>,
        memory_map: &'static [E820MemoryRegion],
    ) -> Self {
        Self {
            memory_map,
            idx: 0,
            current_descriptor: None,
            next_frame: frame,
        }
    }

    fn allocate_frame_from_descriptor(
        &mut self,
        descriptor: E820MemoryRegion,
    ) -> Option<PhysicalFrame<Page4Kb>> {
        let start_addr = descriptor.start();
        let start_frame = PhysicalFrame::<Page4Kb>::containing(start_addr);
        let end_addr = start_addr + descriptor.len();
        let end_frame = PhysicalFrame::<Page4Kb>::containing(end_addr - 1u64);

        // increase self.next_frame to start_frame if smaller
        if self.next_frame.ptr().as_u64() < start_frame.ptr().as_u64() {
            self.next_frame = start_frame;
        }

        if self.next_frame.ptr().as_u64() < end_frame.ptr().as_u64() {
            let ret = self.next_frame;
            self.next_frame = PhysicalFrame::containing(self.next_frame.ptr() + Page4Kb);
            Some(ret)
        } else {
            None
        }
    }

    /// Returns the number of memory regions in the underlying memory map.
    ///
    /// The function always returns the same value, i.e. the length doesn't
    /// change after calls to `allocate_frame`.
    pub fn len(&self) -> usize {
        self.memory_map.len()
    }

    /// Returns the largest detected physical memory address.
    ///
    /// Useful for creating a mapping for all physical memory.
    pub fn max_phys_addr(&self) -> PhysicalAddr {
        self.memory_map
            .iter()
            .map(|r| (r.start() + r.len()).as_u64())
            .max()
            .map(PhysicalAddr::new)
            .unwrap()
    }

    /// Converts this type to a boot info memory map.
    ///
    /// The memory map is placed in the given `regions` slice. The length of the given slice
    /// must be at least the value returned by [`len`] pluse 1.
    ///
    /// The return slice is a subslice of `regions`, shortened to the actual number of regions.
    pub fn construct_memory_map(
        self,
        regions: &mut [MaybeUninit<MemoryRegion>],
    ) -> &mut [MemoryRegion] {
        let mut next_index = 0;

        for descriptor in self.memory_map {
            let mut start = descriptor.start();
            let end = start + descriptor.len();
            let next_free = self.next_frame.ptr();
            let kind = match descriptor.kind() {
                MemoryRegionKind::Usable => {
                    if end.as_u64() <= next_free.as_u64() {
                        MemoryRegionKind::Bootloader
                    } else if descriptor.start().as_u64() >= next_free.as_u64() {
                        MemoryRegionKind::Usable
                    } else {
                        // part of the region is used -> add it separately
                        let used_region = MemoryRegion {
                            start: descriptor.start().as_u64(),
                            end: next_free.as_u64(),
                            kind: MemoryRegionKind::Bootloader,
                        };
                        Self::add_region(used_region, regions, &mut next_index)
                            .expect("Failed to add memory region");

                        // add unused part normally
                        start = next_free;
                        MemoryRegionKind::Usable
                    }
                }
                other => other,
            };

            let region = MemoryRegion {
                start: start.as_u64(),
                end: end.as_u64(),
                kind,
            };
            Self::add_region(region, regions, &mut next_index).unwrap();
        }

        let initialized = &mut regions[..next_index];
        unsafe { MaybeUninit::slice_assume_init_mut(initialized) }
    }

    fn add_region(
        region: MemoryRegion,
        regions: &mut [MaybeUninit<MemoryRegion>],
        next_index: &mut usize,
    ) -> Result<(), ()> {
        unsafe {
            regions
                .get_mut(*next_index)
                .ok_or(())?
                .as_mut_ptr()
                .write(region)
        };
        *next_index += 1;
        Ok(())
    }

    fn next_entry(&mut self) -> Option<E820MemoryRegion> {
        let ret = self.memory_map.get(self.idx);
        if ret.is_some() {
            self.idx += 1;
        }
        ret.copied()
    }
}

impl FrameAllocator<Page4Kb> for LegacyFrameAllocator {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        if let Some(current_descriptor) = self.current_descriptor {
            match self.allocate_frame_from_descriptor(current_descriptor) {
                Some(frame) => return Ok(frame),
                None => self.current_descriptor = None,
            }
        }

        // find next suitable descriptor
        while let Some(descriptor) = self.next_entry() {
            if descriptor.kind() != MemoryRegionKind::Usable {
                continue;
            }
            if let Some(frame) = self.allocate_frame_from_descriptor(descriptor) {
                self.current_descriptor = Some(descriptor);
                return Ok(frame);
            }
        }

        Err(FrameError::Alloc)
    }
}
