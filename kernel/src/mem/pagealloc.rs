use kalloc::AllocatorBin;

use libx64::{
    address::PhysicalAddr,
    paging::{
        frame::{FrameAllocator, FrameError, FrameRange, PhysicalFrame},
        Page4Kb,
    },
};

use alloc::alloc::{Allocator, Layout};

use bootloader::boot_info::{MemoryRegionKind, MemoryRegions};

use crate::mem::pmm::PhysicalMemoryManager;

#[repr(C)]
#[repr(align(512))]
struct PreAlloc([u8; 512 * 4]);

static mut PREALLOC: PreAlloc = PreAlloc([0; 512 * 4]);

static mut BINS: [AllocatorBin; 2048] = [AllocatorBin::new(); 2048];

/// A [`FrameAllocator`] that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    alloc: PhysicalMemoryManager,
}

impl FrameAllocator<Page4Kb> for BootInfoFrameAllocator {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        self.alloc
            .allocate(Layout::new::<[u8; 512]>())
            .map_err(|_err| FrameError::Alloc)
            .map(|ptr| PhysicalAddr::from_ptr(ptr.as_ptr() as *mut u8))
            .map(PhysicalFrame::containing)
    }
}

impl BootInfoFrameAllocator {
    pub fn init(memory_map: &'static MemoryRegions) -> Self {
        let mut iter = memory_map
            .iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .map(|r| {
                // TODO: this is wrong it should be a non inclusive range
                FrameRange::<Page4Kb>::new_addr(
                    PhysicalAddr::new(r.start),
                    PhysicalAddr::new(r.end),
                )
            });

        let page = iter.next().unwrap();

        let vec = unsafe {
            FrameRange::<Page4Kb>::new(
                PhysicalFrame::containing(PhysicalAddr::from_ptr(PREALLOC.0.as_ptr())),
                PhysicalFrame::containing(PhysicalAddr::from_ptr(
                    PREALLOC.0.as_ptr().add(8 * 512 * 4),
                )),
            )
        };
        dbg!(page.len());
        let mut alloc = PhysicalMemoryManager::new();
        alloc.init(vec, unsafe { &mut BINS[..] }, page);
        dbg!("HERE");
        unsafe {
            let idx = &BINS[..]
                .iter()
                .enumerate()
                .find(|(_, bin)| !bin.flags.contains(kalloc::AllocatorBinFlags::USED))
                .map(|(i, _)| i);
            trace!("{:?} wasted bins", idx);
            trace!("{:?} allocated buddies", alloc.at);
        }
        // dbg!(&*alloc.buddies.as_ref().unwrap()[0].lock());
        BootInfoFrameAllocator { alloc }
    }
}
