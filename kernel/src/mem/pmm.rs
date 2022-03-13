use bootloader::boot_info::{MemoryRegionKind, MemoryRegions};
use kalloc::{buddy::BuddyAllocator, AllocatorBin};

use libx64::{
    address::PhysicalAddr,
    paging::{
        frame::{FrameAllocator, FrameError, FrameRange, PhysicalFrame},
        Page1Gb, Page2Mb, Page4Kb,
    },
};

use alloc::{
    alloc::{Allocator, Layout},
    boxed::Box,
    vec::Vec,
};

use kcore::sync::SpinMutex;

use core::iter::Step;

type InnerAllocator = SpinMutex<BuddyAllocator<'static, { Page4Kb as usize }>>;

const PREALLOC_LEN: usize = 512 * 4; // 16Kb for the initial allocator
const BINS_LEN: usize = 2048;

#[repr(C)]
#[repr(align(512))]
struct PreAlloc([u8; PREALLOC_LEN]);
static mut PREALLOC: PreAlloc = PreAlloc([0; PREALLOC_LEN]);

static mut BINS: [AllocatorBin; BINS_LEN] = [AllocatorBin::new(); BINS_LEN];

pub struct PhysicalMemoryManager {
    pub buddies: Option<Box<[InnerAllocator]>>,
    pub at: usize,
}

impl PhysicalMemoryManager {
    pub const fn new() -> Self {
        Self {
            buddies: None,
            at: 0,
        }
    }

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
                    PREALLOC.0.as_ptr().add(PREALLOC_LEN * u8::BITS as usize),
                )),
            )
        };

        // dbg!(page.len()); NOTE: this makes the allocator crash ???
        let mut alloc = Self::new();
        alloc.populate_buddies(vec, unsafe { &mut BINS[..] }, page);
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
        alloc
    }

    fn populate_buddies(
        &mut self,
        vec: FrameRange<Page4Kb>,
        bins: &'static mut [AllocatorBin],
        range: FrameRange<Page4Kb>,
    ) {
        let mut buddies = unsafe {
            Vec::from_raw_parts(
                vec.start().ptr::<InnerAllocator>().unwrap().as_ptr(),
                (vec.len() * (Page4Kb as usize)) / (8 * Layout::new::<InnerAllocator>().size()),
                (vec.len() * (Page4Kb as usize)) / (8 * Layout::new::<InnerAllocator>().size()),
            )
            .into_boxed_slice()
        };

        let mut it = range.step_by(64).zip(bins.array_chunks_mut::<64>());

        for i in 0..buddies.len() {
            let (frame, chunk) = match it.next() {
                Some(a) => a,
                None => {
                    self.at = i;
                    break;
                }
            };
            let range = FrameRange::new(frame, Step::forward_checked(frame, 64).unwrap());
            buddies[i] = SpinMutex::new(BuddyAllocator::new(chunk, range).unwrap());
        }

        self.buddies = Some(buddies);
    }
}

unsafe impl Allocator for PhysicalMemoryManager {
    fn allocate(
        &self,
        layout: Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        let buds = self.buddies.as_ref().ok_or(core::alloc::AllocError)?;
        for bud in &buds[..self.at] {
            match bud.allocate(layout) {
                Ok(a) => return Ok(a),
                Err(_) => continue,
            }
        }
        Err(core::alloc::AllocError)
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: Layout) {
        let buds = self
            .buddies
            .as_ref()
            .ok_or(core::alloc::AllocError)
            .expect("allocator not initialized");
        for bud in &buds[..self.at] {
            if bud.lock().contains(ptr.as_ptr()) {
                bud.deallocate(ptr, layout);
            }
        }
    }
}

impl FrameAllocator<Page4Kb> for PhysicalMemoryManager {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        self.allocate(Layout::new::<[u8; 512]>())
            .map_err(|_err| FrameError::Alloc)
            .map(|ptr| PhysicalAddr::from_ptr(ptr.as_ptr() as *mut u8))
            .map(PhysicalFrame::containing)
    }
}

impl FrameAllocator<Page2Mb> for PhysicalMemoryManager {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page2Mb>, FrameError> {
        self.allocate(Layout::new::<[u8; 512 * 512 * 2]>())
            .map_err(|_err| FrameError::Alloc)
            .map(|ptr| PhysicalAddr::from_ptr(ptr.as_ptr() as *mut u8))
            .map(PhysicalFrame::containing)
    }
}

impl FrameAllocator<Page1Gb> for PhysicalMemoryManager {
    fn alloc(&mut self) -> Result<PhysicalFrame<Page1Gb>, FrameError> {
        self.allocate(Layout::new::<[u8; 512 * 512 * 2]>())
            .map_err(|_err| FrameError::Alloc)
            .map(|ptr| PhysicalAddr::from_ptr(ptr.as_ptr() as *mut u8))
            .map(PhysicalFrame::containing)
    }
}
