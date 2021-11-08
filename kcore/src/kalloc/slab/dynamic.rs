use alloc::alloc::Allocator;

use core::{
    alloc::{AllocError, Layout},
    num::NonZeroU16,
    ops::Range,
    ptr::NonNull,
};

use libx64::{
    address::VirtualAddr,
    paging::{page::PageRange, Page4Kb},
};

use crate::kalloc::slab::{SlabCheck, SlabSize};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
struct Bucket<const MIN: usize>(NonZeroU16);

impl<const MIN: usize> core::fmt::Debug for Bucket<MIN>
where
    SlabCheck<MIN>: SlabSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bucket")
            .field("size", &self.size())
            .field("range", &self.range())
            .finish()
    }
}

impl<const MIN: usize> Bucket<MIN>
where
    SlabCheck<MIN>: SlabSize,
{
    const fn new(start: u16, end: u16) -> Self {
        unsafe { Self(NonZeroU16::new_unchecked(end << 8 | start)) }
    }

    const fn split(self) -> (Bucket<MIN>, Bucket<MIN>) {
        let (start, end) = (self.range().start as u16, self.range().end as u16);
        (
            Self::new(start, start + (end - start) / 2),
            Self::new(start + (end - start) / 2, end),
        )
    }

    unsafe fn merge(self, rhs: Self) -> Self {
        let start = core::cmp::min(self.start(), rhs.start()) as u16;
        let end = core::cmp::max(self.end(), rhs.end()) as u16;
        Self::new(start, end)
    }

    const fn start(&self) -> usize {
        (self.0.get() & 0b0000_0000_1111_1111) as usize
    }
    const fn end(&self) -> usize {
        ((self.0.get() & 0b1111_1111_0000_0000) >> 8) as usize
    }

    const fn range(&self) -> Range<usize> {
        self.start()..self.end()
    }

    const fn size(&self) -> usize {
        (self.end() - self.start()) * MIN
    }

    const fn size_bytes(&self) -> usize {
        self.size() / 8
    }
}

#[repr(C)]
pub struct Slab<const MIN: usize> {
    bins: [Option<Bucket<MIN>>; 32],
    start: VirtualAddr,
    used_mask: u32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Error {
    InvalidBucketSize,
    InvalidPageRange { expected: u64, got: u64 },
}

impl<const MIN: usize> Slab<MIN>
where
    SlabCheck<MIN>: SlabSize,
{
    pub const fn new(page: PageRange<Page4Kb>) -> Result<Self, Error> {
        if !MIN.is_power_of_two() {
            return Err(Error::InvalidBucketSize);
        }
        let expected = (MIN as u64 * 32) / Page4Kb;
        let got = page.len() as u64;

        if expected != got {
            return Err(Error::InvalidPageRange { expected, got });
        }

        let start = page.start();

        let mut this = Self {
            bins: [None; 32],
            used_mask: 0,
            start,
        };
        this.bins[0] = Some(Bucket::new(0, 32));
        Ok(this)
    }

    pub const fn len(&self) -> usize {
        self.used_mask.count_ones() as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn have_buckets_for(&self, size: usize) -> bool {
        !(match size {
            a if a == MIN * 32 => {
                (self.used_mask & 0b0000_0000_0000_0000_0000_0000_0000_0001).count_ones() == 1
            }
            a if a == MIN * 16 => {
                (self.used_mask & 0b0000_0000_0000_0001_0000_0000_0000_0001).count_ones() == 2
            }
            a if a == MIN * 8 => {
                (self.used_mask & 0b0000_0001_0000_0001_0000_0001_0000_0001).count_ones() == 4
            }
            a if a == MIN * 4 => {
                (self.used_mask & 0b0001_0001_0001_0001_0001_0001_0001_0001).count_ones() == 8
            }
            a if a == MIN * 2 => {
                (self.used_mask & 0b0101_0101_0101_0101_0101_0101_0101_0101).count_ones() == 16
            }
            a if a <= MIN => self.used_mask.count_ones() == 32,
            _ => true,
        })
    }

    const fn is_used(&self, idx: usize) -> bool {
        self.used_mask & (1 << idx) != 0
    }
    const fn available_for(&self, idx: usize, layout: Layout) -> bool {
        let size = layout.size();
        matches!(self.bins[idx], Some(bin) if size <= bin.size_bytes() && !self.is_used(idx))
    }

    fn mark_used(&mut self, idx: usize) {
        self.used_mask |= 1 << idx;
    }
    fn mark_unused(&mut self, idx: usize) {
        self.used_mask &= !(1 << idx);
    }

    fn split_at(&mut self, idx: usize) -> Option<(usize, usize)> {
        let used = self.is_used(idx);
        let bin = self.bins.get_mut(idx).and_then(|bin| match bin {
            // Bin is not splitable
            Some(e) if used || e.size() <= MIN => None,
            a @ Some(_) => a.take(),
            None => None,
        })?;

        let (left, right) = bin.split();

        self.bins[left.start()] = Some(left);
        self.bins[right.start()] = Some(right);
        Some((left.start(), right.start()))
    }

    // no usage check
    unsafe fn split_at_unchecked(&mut self, idx: usize) -> Option<(usize, usize)> {
        let bin = self.bins.get_mut(idx).and_then(|bin| match bin {
            // Bin is not splitable
            Some(e) if e.size() <= MIN => None,
            a @ Some(_) => a.take(),
            None => None,
        })?;

        let (left, right) = bin.split();

        self.bins[left.start()] = Some(left);
        self.bins[right.start()] = Some(right);
        Some((left.start(), right.start()))
    }

    const unsafe fn addr_for(&self, range: Range<usize>) -> *mut u8 {
        let start = range.start;
        (self.start.as_u64() + (start * MIN) as u64) as *mut u8
    }

    fn bin_for(&self, ptr: NonNull<u8>) -> Option<&Bucket<MIN>> {
        let idx = (ptr.as_ptr() as u64 - self.start.as_u64()) as usize / MIN;
        self.bins[idx].as_ref()
    }

    const fn buddy_of(range: Range<usize>) -> Range<usize> {
        let diff = range.end - range.start;

        if (range.start / diff) % 2 == 0 {
            range.end..(range.end + diff)
        } else {
            (range.start - diff)..range.start
        }
    }

    const fn is_right(range: Range<usize>) -> bool {
        (range.start / (range.end - range.start)) % 2 != 0
    }
}

unsafe impl<const MIN: usize> Allocator for crate::sync::SpinMutex<Slab<MIN>>
where
    SlabCheck<MIN>: SlabSize,
{
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // Out of memory
        let size = core::cmp::max(layout.size().next_power_of_two() * 8, MIN as usize);

        let mut this = self.lock();
        // fast path
        if !this.have_buckets_for(size) {
            return Err(AllocError);
        }

        match slot_range(size).find(|&i| this.available_for(i, layout)) {
            Some(i) => {
                let mut bin = this.bins[i].ok_or(AllocError)?;

                while size <= bin.size() / 2 {
                    let (start, _) = this.split_at(bin.start()).ok_or(AllocError)?;
                    bin = this.bins[start].ok_or(AllocError)?;
                }

                this.mark_used(i);

                Ok(unsafe {
                    NonNull::new_unchecked(core::slice::from_raw_parts_mut(
                        this.addr_for(bin.range()),
                        bin.size_bytes(),
                    ))
                })
            }
            None => Err(AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let mut this = self.lock();

        let range = match this.bin_for(ptr) {
            Some(bin) if layout.size() <= bin.size() => bin.range(),
            _ => panic!("pointer is at an invalid bin or doesn't belong to this allocator"),
        };

        let mut index = range.start;
        this.mark_unused(index);

        // Merge the current bin with its buddy if possible.
        // Merging can be be achieved if the theoretical buddy follows:
        //  1. It is in bin range (< 32)
        //  2. Buckets have the same size (bin.size() == buddy.size())
        //  3. Buddy is not used
        let mut buddy_range = Slab::<MIN>::buddy_of(range);
        while buddy_range.start < 32
            && this.bins[index].as_ref().map(Bucket::size)
                == this.bins[buddy_range.start].as_ref().map(Bucket::size)
            && !this.is_used(buddy_range.start)
        {
            let buddy = this.bins[buddy_range.start]
                .take()
                .expect("buddy should exist");
            let bin = this.bins[index].take().expect("bin should exist");
            let new = bin.merge(buddy);
            index = new.range().start;

            this.bins[index] = Some(new);

            buddy_range = Slab::<MIN>::buddy_of(new.range());
        }
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );
        // TODO: what is the default way of handling growing errors? Should it deallocate the old
        // ptr ?

        // Out of memory
        if new_layout.size() > (32 * MIN) {
            return Err(AllocError);
        }
        // fast path if the allocs are the same
        if new_layout.size() == old_layout.size() {
            return Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
                ptr.as_ptr(),
                new_layout.size(),
            )));
        }

        let mut this = self.lock();

        let range = match this.bin_for(ptr) {
            Some(bin) if old_layout.size() <= bin.size() => bin.range(),
            _ => panic!("pointer is at an invalid bin or doesn't belong to this allocator"),
        };
        let index = range.start;

        let buddy_range = Slab::<MIN>::buddy_of(range);
        let buddy_index = buddy_range.start;

        // Fast path:
        //
        // Merge the two blocks without copying anything
        //
        // 1.
        //             |
        //          (Parent)
        //          /       \
        //      Bin         Buddy(unused)
        // 2.
        //             |
        //            Bin
        //
        if Slab::<MIN>::is_right(buddy_range) && !this.is_used(buddy_index) {
            let buddy = this.bins[buddy_index].take().expect("buddy should exist");
            let bin = this.bins[index].take().expect("bin should exist");
            let new = bin.merge(buddy);
            this.bins[index] = Some(new);

            return Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
                ptr.as_ptr(),
                new_layout.size(),
            )));
        }

        let new_ptr = self.allocate(new_layout)?;

        // SAFETY: because `new_layout.size()` must be greater than or equal to
        // `old_layout.size()`, both the old and new memory allocation are valid for reads and
        // writes for `old_layout.size()` bytes. Also, because the old allocation wasn't yet
        // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
        // safe. The safety contract for `dealloc` must be upheld by the caller.
        core::ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), old_layout.size());
        self.deallocate(ptr, old_layout);

        Ok(new_ptr)
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );
        let new_ptr = self.grow(ptr, old_layout, new_layout)?;
        new_ptr
            .as_non_null_ptr()
            .as_ptr()
            .add(old_layout.size())
            .write_bytes(0, new_layout.size() - old_layout.size());

        Ok(new_ptr)
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );
        let mut this = self.lock();

        let range = match this.bin_for(ptr) {
            Some(bin) if old_layout.size() <= bin.size() => bin.range(),
            _ => panic!("pointer is at an invalid bin or doesn't belong to this allocator"),
        };

        let mut bin = this.bins[range.start].ok_or(AllocError)?;

        // forward the pointer since we can't shrink (noone has too much memory I guess)
        if bin.size() == MIN {
            return Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
                ptr.as_ptr(),
                bin.size_bytes(),
            )));
        }

        let size = core::cmp::max(new_layout.size().next_power_of_two() * 8, MIN as usize);
        while size <= bin.size() / 2 {
            // let (start, _) = this.split_at(bin.start()).ok_or(AllocError)?;
            let (start, _) = this.split_at_unchecked(bin.start()).unwrap();
            bin = this.bins[start].ok_or(AllocError)?;
        }

        this.mark_used(bin.range().start);

        Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
            this.addr_for(bin.range()),
            bin.size_bytes(),
        )))
    }
}

#[inline]
fn slot_range(size: usize) -> core::iter::StepBy<Range<usize>> {
    (0..32).step_by(size / 128)
}

impl<const M: usize> core::fmt::Debug for Slab<M>
where
    SlabCheck<M>: SlabSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Slab")
            .field("bins", &self.bins)
            .field("mask", &format_args!("{:#034b}", &self.used_mask))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_all_slabs() {
        let buddy = crate::sync::SpinMutex::new(
            Slab::<128>::new(PageRange::with_size(
                VirtualAddr::new(0x0000_dead_beaf_0000),
                Page4Kb,
            ))
            .unwrap(),
        );

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        for bin in &buddy.lock().bins[8..24] {
            assert_eq!(bin.as_ref().map(Bucket::size), Some(128))
        }

        assert!(buddy.lock().len() == 32);
    }

    #[test]
    fn allocate_all_big() {
        let buddy = crate::sync::SpinMutex::new(
            Slab::<128>::new(PageRange::with_size(
                VirtualAddr::new(0x0000_dead_beaf_0000),
                Page4Kb,
            ))
            .unwrap(),
        );

        let _ = buddy.allocate(Layout::new::<[u8; 512]>()).unwrap();

        assert!(buddy.lock().len() == 1);
    }

    #[test]
    fn shrink_once() {
        // TODO: expand tests
        let buddy = crate::sync::SpinMutex::new(
            Slab::<128>::new(PageRange::with_size(
                VirtualAddr::new(0x0000_dead_beaf_0000),
                Page4Kb,
            ))
            .unwrap(),
        );

        let old_ptr = buddy.allocate(Layout::new::<[u8; 512]>()).unwrap();

        assert!(buddy.lock().len() == 1);
        assert!(buddy.lock().is_used(0));
        assert_eq!(buddy.lock().bins[0].as_ref().map(Bucket::size), Some(4096));

        let new_ptr = unsafe {
            buddy
                .shrink(
                    old_ptr.cast(),
                    Layout::new::<[u8; 512]>(),
                    Layout::new::<[u8; 256]>(),
                )
                .unwrap()
        };

        assert!(buddy.lock().len() == 1);

        assert!(buddy.lock().is_used(0));
        assert!(!buddy.lock().is_used(16));

        // SAFETY: we never read anything else than the pointer value
        unsafe { assert_eq!(new_ptr.as_ref().as_ptr(), old_ptr.as_ref().as_ptr()) };

        assert_eq!(buddy.lock().bins[0].as_ref().map(Bucket::size), Some(2048));

        assert_eq!(buddy.lock().bins[16].as_ref().map(Bucket::size), Some(2048));
    }

    #[test]
    fn grow_big_fast() {
        let buddy = crate::sync::SpinMutex::new(
            Slab::<128>::new(PageRange::with_size(
                VirtualAddr::new(0x0000_dead_beaf_0000),
                Page4Kb,
            ))
            .unwrap(),
        );

        // this should split the allocator in 3 pieces (2*1024 bits bins, 1*2048 bits bin)
        let ptr = buddy.allocate(Layout::new::<[u8; 128]>()).unwrap();
        assert_eq!(buddy.lock().bins[0].as_ref().map(Bucket::size), Some(1024));
        assert_eq!(buddy.lock().bins[8].as_ref().map(Bucket::size), Some(1024));
        assert_eq!(buddy.lock().bins[16].as_ref().map(Bucket::size), Some(2048));

        unsafe {
            // this should merge the allocator in 2 pieces (2*2048 bits bins)
            let ptr = buddy
                .grow(
                    ptr.cast(),
                    Layout::new::<[u8; 128]>(),
                    Layout::new::<[u8; 256]>(),
                )
                .unwrap();

            assert_eq!(buddy.lock().bins[0].as_ref().map(Bucket::size), Some(2048));
            assert_eq!(buddy.lock().bins[16].as_ref().map(Bucket::size), Some(2048));

            // this should merge the allocator in 1 pieces (1*4096 bits bins)
            let _ = buddy
                .grow(
                    ptr.cast(),
                    Layout::new::<[u8; 256]>(),
                    Layout::new::<[u8; 512]>(),
                )
                .unwrap();

            assert_eq!(buddy.lock().bins[0].as_ref().map(Bucket::size), Some(4096));
        }

        assert!(buddy.lock().len() == 1);
    }

    #[test]
    fn allocate_mixed() {
        let buddy = crate::sync::SpinMutex::new(
            Slab::<128>::new(PageRange::with_size(
                VirtualAddr::new(0x0000_dead_beaf_0000),
                Page4Kb,
            ))
            .unwrap(),
        );

        // 2 - 512 bits slot = 1024 bits
        let _ = buddy.allocate(Layout::new::<[u8; 64]>()).unwrap();
        assert!(buddy.lock().is_used(0));
        assert!(!buddy.lock().is_used(1));
        assert!(!buddy.lock().is_used(2));
        assert!(!buddy.lock().is_used(3));
        assert!(!buddy.lock().is_used(4));

        let _ = buddy.allocate(Layout::new::<[u8; 64]>()).unwrap();
        assert!(buddy.lock().is_used(4));
        assert!(!buddy.lock().is_used(5));
        assert!(!buddy.lock().is_used(6));
        assert!(!buddy.lock().is_used(7));

        // 16 - 128 bits slot = 2048 bits
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate(Layout::new::<u8>()).unwrap();

        for bin in &buddy.lock().bins[8..24] {
            assert_eq!(bin.as_ref().map(Bucket::size), Some(128))
        }

        // 1 - 1024 bits slot
        let _ = buddy.allocate(Layout::new::<[u8; 128]>()).unwrap();
        assert!(buddy.lock().is_used(24));
        assert!(!buddy.lock().is_used(25));
        assert!(!buddy.lock().is_used(26));
        assert!(!buddy.lock().is_used(27));
        assert!(!buddy.lock().is_used(28));
        assert!(!buddy.lock().is_used(29));
        assert!(!buddy.lock().is_used(30));
        assert!(!buddy.lock().is_used(31));

        assert!(buddy.lock().len() == 19);
    }

    #[test]
    fn deallocate_all_slabs() {
        let buddy = crate::sync::SpinMutex::new(
            Slab::<128>::new(PageRange::with_size(
                VirtualAddr::new(0x0000_dead_beaf_0000),
                Page4Kb,
            ))
            .unwrap(),
        );

        let allocs = [
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
            buddy.allocate(Layout::new::<u8>()).unwrap(),
        ];
        assert_eq!(allocs.len(), 32);

        for alloc in allocs {
            unsafe {
                buddy.deallocate(alloc.cast(), Layout::new::<u8>());
            }
        }
        assert_eq!(buddy.lock().bins[0].as_ref().map(Bucket::size), Some(4096));
        for bin in &buddy.lock().bins[1..] {
            assert!(matches!(bin, None));
        }

        assert!(buddy.lock().len() == 0);
    }

    #[test]
    fn buddy_of_test() {
        assert_eq!(Slab::<128>::buddy_of(0..1), 1..2);
        assert_eq!(Slab::<128>::buddy_of(1..2), 0..1);

        assert_eq!(Slab::<128>::buddy_of(2..3), 3..4);
        assert_eq!(Slab::<128>::buddy_of(3..4), 2..3);

        assert_eq!(Slab::<128>::buddy_of(4..5), 5..6);
        assert_eq!(Slab::<128>::buddy_of(5..6), 4..5);

        assert_eq!(Slab::<128>::buddy_of(0..2), 2..4);
        assert_eq!(Slab::<128>::buddy_of(2..4), 0..2);

        assert_eq!(Slab::<128>::buddy_of(0..4), 4..8);
        assert_eq!(Slab::<128>::buddy_of(4..8), 0..4);

        assert_eq!(Slab::<128>::buddy_of(0..8), 8..16);
        assert_eq!(Slab::<128>::buddy_of(8..16), 0..8);

        assert_eq!(Slab::<128>::buddy_of(0..16), 16..32);
        assert_eq!(Slab::<128>::buddy_of(16..32), 0..16);
    }
}
