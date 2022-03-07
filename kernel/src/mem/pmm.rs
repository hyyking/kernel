use kalloc::{buddy::BuddyAllocator, AllocatorBin};

use libx64::paging::{frame::FrameRange, Page4Kb};

use alloc::{alloc::Layout, boxed::Box, vec::Vec};

use kcore::sync::SpinMutex;

use core::iter::Step;

type InnerAllocator = SpinMutex<BuddyAllocator<'static, { Page4Kb as usize }>>;

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

    pub fn init(
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

unsafe impl alloc::alloc::Allocator for PhysicalMemoryManager {
    fn allocate(
        &self,
        layout: Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        let buds = self.buddies.as_ref().ok_or(core::alloc::AllocError)?;
        for bud in &buds[..self.at] {
            // TODO: removing this crashes the kernel ??? needs debuging
            dbg!("it works");
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
