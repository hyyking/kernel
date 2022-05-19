use core::mem::MaybeUninit;

use crate::boot_info::{MemoryRegion, MemoryRegionKind};

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{
        frame::{FrameAllocator, FrameError, PhysicalFrame},
        Page4Kb,
    },
};

/// A physical memory region returned by an `e820` BIOS call.
///
/// See http://wiki.osdev.org/Detecting_Memory_(x86)#Getting_an_E820_Memory_Map for more info.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct E820MemoryRegion {
    /// start address
    pub start_addr: u64,
    /// length in bits
    pub len: u64,
    /// region type as u32 see the method
    pub region_type: u32,
    /// acpi extend attributes
    pub acpi_extended_attributes: u32,
}

#[derive(Debug)]
/// E820 MemoryMap abstraction
pub struct E820MemoryMap<'a> {
    /// E820MemoryMap
    pub memory_map: &'a [E820MemoryRegion],
    /// next available frame
    pub next_frame: PhysicalFrame<Page4Kb>,
}

#[derive(Debug)]
/// A physical frame allocator based on a BIOS provided memory map.
pub struct BiosFrameAllocator<'a> {
    /// E820MemoryMap
    memory_map: E820MemoryMap<'a>,
    current_descriptor: Option<E820MemoryRegion>,
    idx: usize,
}

impl E820MemoryRegion {
    /// Return the start pointer of the region
    pub const fn start(&self) -> PhysicalAddr {
        PhysicalAddr::new(self.start_addr)
    }

    /// Return the bit length of the region
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Return the kind of the region
    pub const fn kind(&self) -> MemoryRegionKind {
        match self.region_type {
            1 => MemoryRegionKind::Usable,
            other => MemoryRegionKind::UnknownBios(other),
        }
    }
}

impl<'a> E820MemoryMap<'a> {
    /// Create a E820MemoryMap from a pointer in virtual memory
    pub unsafe fn from_memory(
        addr: VirtualAddr,
        len: usize,
        next_frame: PhysicalFrame<Page4Kb>,
    ) -> Self {
        let memory_map = unsafe {
            let ptr = addr.ptr::<E820MemoryRegion>().unwrap().as_ptr();
            core::slice::from_raw_parts(ptr, len)
        };
        Self {
            memory_map,
            next_frame,
        }
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

    /// Number of entries in the memory_map
    pub const fn len(&self) -> usize {
        self.memory_map.len()
    }
}

impl<'a> BiosFrameAllocator<'a> {
    /// Creates a new frame allocator based on the given legacy memory regions. Skips any frames
    /// before the given `frame`.
    pub const fn new(memory_map: E820MemoryMap<'a>) -> Result<Self, FrameError> {
        Ok(Self {
            memory_map,
            idx: 0,
            current_descriptor: None,
        })
    }

    /// Returns the number of memory regions in the underlying memory map.
    ///
    /// The function always returns the same value, i.e. the length doesn't
    /// change after calls to `allocate_frame`.
    pub const fn len(&self) -> usize {
        self.memory_map.len()
    }

    /// References the underlying memory map
    pub const fn memory_map(&self) -> &E820MemoryMap<'a> {
        &self.memory_map
    }

    /// Consume the allocator to return the memory map, useful to create bootinfo memory map
    pub const fn into_memory_map(self) -> E820MemoryMap<'a> {
        self.memory_map
    }

    fn next_entry(&mut self) -> Option<E820MemoryRegion> {
        let ret = self.memory_map.memory_map.get(self.idx);
        if ret.is_some() {
            self.idx += 1;
        }
        ret.copied()
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
        if self.memory_map.next_frame.ptr().as_u64() < start_frame.ptr().as_u64() {
            self.memory_map.next_frame = start_frame;
        }

        if self.memory_map.next_frame.ptr().as_u64() < end_frame.ptr().as_u64() {
            let ret = self.memory_map.next_frame;
            self.memory_map.next_frame =
                PhysicalFrame::containing(self.memory_map.next_frame.ptr() + Page4Kb);
            Some(ret)
        } else {
            None
        }
    }
}

impl<'a> FrameAllocator<Page4Kb> for BiosFrameAllocator<'a> {
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

pub trait BootFrameAllocator: libx64::paging::frame::FrameAllocator<Page4Kb> {
    fn write_memory_map<'a>(
        &self,
        mem: &'a mut [MaybeUninit<MemoryRegion>],
    ) -> Result<&'a mut [MemoryRegion], ()>;

    fn max_physical_address(&self) -> PhysicalAddr;

    fn len(&self) -> usize;
}

impl<'a> BootFrameAllocator for BiosFrameAllocator<'a> {
    fn write_memory_map<'b>(
        &self,
        mem: &'b mut [MaybeUninit<MemoryRegion>],
    ) -> Result<&'b mut [MemoryRegion], ()> {
        construct_memory_map(self.memory_map(), mem)
    }

    fn max_physical_address(&self) -> PhysicalAddr {
        self.memory_map().max_phys_addr()
    }

    fn len(&self) -> usize {
        self.len()
    }
}

/// Converts this type to a boot info memory map.
///
/// The memory map is placed in the given `regions` slice. The length of the given slice
/// must be at least the value returned by [`len`] pluse 1.
///
/// The return slice is a subslice of `regions`, shortened to the actual number of regions.
pub fn construct_memory_map<'a>(
    mem: &E820MemoryMap<'_>,
    regions: &'a mut [MaybeUninit<MemoryRegion>],
) -> Result<&'a mut [MemoryRegion], ()> {
    let mut next_index = 0;

    for descriptor in mem.memory_map {
        let mut start = descriptor.start();
        let end = start + descriptor.len();
        let next_free = mem.next_frame.ptr();
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
                    add_region(used_region, regions, &mut next_index)?;

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
        add_region(region, regions, &mut next_index)?;
    }

    let initialized = &mut regions[..next_index];
    Ok(unsafe { MaybeUninit::slice_assume_init_mut(initialized) })
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
