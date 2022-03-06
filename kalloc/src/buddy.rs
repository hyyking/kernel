use alloc::alloc::Allocator;

use core::{
    alloc::{AllocError, Layout},
    ops::Range,
    ptr::NonNull,
};

use libx64::{
    address::VirtualAddr,
    paging::{page::PageRangeInclusive, Page4Kb},
};

const DEPTH: usize = 2usize.pow(6);

type MaskInt = u64;
type BucketInt = u32;

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct Bucket<const MIN: usize>(BucketInt);

impl<const MIN: usize> Bucket<MIN> {
    const fn new(start: BucketInt, end: BucketInt) -> Self {
        Self(end << (BucketInt::BITS / 2) | start)
    }
    pub const fn empty() -> Self {
        Self(0)
    }

    const fn is_empty(&self) -> bool {
        self.start() == 0 && self.end() == 0
    }

    const fn start(&self) -> usize {
        // create a mask of half the bottom bits
        const MASK: BucketInt = (BucketInt::MAX << (BucketInt::BITS / 2)) >> (BucketInt::BITS / 2);
        (self.0 & MASK) as usize
    }
    const fn end(&self) -> usize {
        const MASK: BucketInt = (BucketInt::MAX >> (BucketInt::BITS / 2)) << (BucketInt::BITS / 2);
        // create a mask of half the upper bits and shift the index stored in the
        // upper bits back down
        (self.0 & MASK >> (BucketInt::BITS / 2)) as usize
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

    const fn split(self) -> (Bucket<MIN>, Bucket<MIN>) {
        let (start, end) = (self.start() as BucketInt, self.end() as BucketInt);
        (
            Self::new(start, start + (end - start) / 2),
            Self::new(start + (end - start) / 2, end),
        )
    }

    unsafe fn merge(self, rhs: Self) -> Self {
        let start = core::cmp::min(self.start(), rhs.start()) as BucketInt;
        let end = core::cmp::max(self.end(), rhs.end()) as BucketInt;
        Self::new(start, end)
    }

    fn take(&mut self) -> Option<Self> {
        if self.is_empty() {
            None
        } else {
            let ret = self.clone();
            *self = Self::empty();
            Some(ret)
        }
    }
}

impl<const MIN: usize> Bucket<MIN> {}

impl<const MIN: usize> core::fmt::Debug for Bucket<MIN> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bucket")
            .field("size", &self.size())
            .field("range", &self.range())
            .finish()
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct BinSlice<'a, const MIN: usize> {
    bins: &'a mut [Bucket<MIN>],
}

impl<'a, const MIN: usize> BinSlice<'a, MIN> {
    unsafe fn set_unchecked(&mut self, index: usize, bucket: Bucket<MIN>) {
        self.bins[index] = bucket;
    }

    fn get(&self, index: usize) -> Option<&Bucket<MIN>> {
        let bin = self.bins.get(index)?;
        if bin.is_empty() {
            None
        } else {
            Some(bin)
        }
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut Bucket<MIN>> {
        let bin = self.bins.get_mut(index)?;
        if bin.is_empty() {
            None
        } else {
            Some(bin)
        }
    }

    #[cfg(test)]
    fn get_range<U>(&self, range: U) -> &[Bucket<MIN>]
    where
        U: core::slice::SliceIndex<[Bucket<MIN>], Output = [Bucket<MIN>]>,
    {
        &self.bins[range]
    }
}

#[repr(C)]
pub struct BuddyAllocator<'a, const MIN: usize> {
    bins: BinSlice<'a, MIN>,
    start: VirtualAddr,
    used_mask: MaskInt,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Error {
    InvalidPowerOfTwo,
    InvalidBinSliceSize,
    InvalidPageRange { expected: usize, got: usize },
}

impl<'a, const MIN: usize> BuddyAllocator<'a, MIN> {
    pub fn new(
        bins: &'a mut [Bucket<MIN>],
        page: PageRangeInclusive<Page4Kb>,
    ) -> Result<Self, Error> {
        if !MIN.is_power_of_two() {
            return Err(Error::InvalidPowerOfTwo);
        }
        if bins.len() != DEPTH {
            return Err(Error::InvalidBinSliceSize);
        }
        let expected = (MIN * DEPTH) / (Page4Kb as usize);
        let got = page.len();

        if expected != got {
            return Err(Error::InvalidPageRange { expected, got });
        }

        let start = page.start();

        let mut this = Self {
            bins: BinSlice { bins },
            used_mask: 0,
            start,
        };
        // SAFETY: All the bins start as empty
        unsafe {
            this.bins
                .set_unchecked(0, Bucket::new(0, DEPTH as BucketInt));
        }
        Ok(this)
    }

    pub const fn len(&self) -> usize {
        self.used_mask.count_ones() as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn have_buckets_for(&self, size: usize) -> bool {
        debug_assert!(size.is_power_of_two());

        const fn buckets_size_mask(size: MaskInt) -> MaskInt {
            let mut i: MaskInt = 0;
            let mut mask: MaskInt = 0;
            while i < MaskInt::BITS as MaskInt {
                mask |= 1 << i;
                i += size;
            }
            mask
        }

        let level = size as MaskInt / MIN as MaskInt;
        let ones = MaskInt::BITS as MaskInt / level;
        !((self.used_mask & buckets_size_mask(level)).count_ones() == ones as u32)
    }

    const fn is_used(&self, idx: usize) -> bool {
        self.used_mask & (1 << idx) != 0
    }
    fn available_for(&self, idx: usize, layout: Layout) -> bool {
        matches!(self.bins.get(idx), Some(bin) if layout.size() <= bin.size_bytes() && !self.is_used(idx))
    }

    fn mark_used(&mut self, idx: usize) {
        self.used_mask |= 1 << idx;
    }
    fn mark_unused(&mut self, idx: usize) {
        self.used_mask &= !(1 << idx);
    }

    fn split_at(&mut self, idx: usize) -> Option<(usize, usize)> {
        let used = self.is_used(idx);
        let bin = self.bins.get_mut(idx).and_then(|bin| {
            if used || bin.size() <= MIN {
                return None;
            } else {
                bin.take()
            }
        })?;

        let (left, right) = bin.split();

        // SAFETY: TODO
        unsafe {
            self.bins.set_unchecked(left.start(), left);
            self.bins.set_unchecked(right.start(), right);
        }
        Some((left.start(), right.start()))
    }

    // no usage check
    unsafe fn split_at_unchecked(&mut self, idx: usize) -> Option<(usize, usize)> {
        let bin = self.bins.get_mut(idx).and_then(|bin| {
            if bin.size() <= MIN {
                return None;
            } else {
                bin.take()
            }
        })?;

        let (left, right) = bin.split();

        // SAFETY: TODO
        self.bins.set_unchecked(left.start(), left);
        self.bins.set_unchecked(right.start(), right);
        Some((left.start(), right.start()))
    }

    const unsafe fn addr_for(&self, range: Range<usize>) -> *mut u8 {
        let start = range.start;
        (self.start.as_u64() + (start * MIN) as u64) as *mut u8
    }

    fn bin_for(&self, ptr: NonNull<u8>) -> Option<&Bucket<MIN>> {
        let idx = (ptr.as_ptr() as u64 - self.start.as_u64()) as usize / MIN;
        self.bins.get(idx)
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

unsafe impl<const MIN: usize> Allocator for BuddyAllocator<'_, MIN> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // Out of memory
        let size = core::cmp::max(layout.size().next_power_of_two() * 8, MIN as usize);

        // FIXME: this is wrong on so many levels
        #[allow(unsafe_op_in_unsafe_fn, unused_unsafe)]
        let this = unsafe { &mut *(self as *const _ as usize as *mut Self) };
        // fast path
        //
        if !this.have_buckets_for(size) {
            return Err(AllocError);
        }

        match slot_range::<MIN>(size).find(|&i| this.available_for(i, layout)) {
            Some(i) => {
                let mut bin = this.bins.get(i).ok_or(AllocError)?.clone();

                while size <= bin.size() / 2 {
                    let (start, _) = this.split_at(bin.start()).ok_or(AllocError)?;
                    bin = *this.bins.get(start).ok_or(AllocError)?;
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
        // FIXME: this is wrong on so many levels
        #[allow(unsafe_op_in_unsafe_fn, unused_unsafe)]
        let this = unsafe { &mut *(self as *const _ as usize as *mut Self) };

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
        let mut buddy_range = Self::buddy_of(range);
        while buddy_range.start < DEPTH
            && this.bins.get(index).map(Bucket::size)
                == this.bins.get(buddy_range.start).map(Bucket::size)
            && !this.is_used(buddy_range.start)
        {
            let buddy = this
                .bins
                .get_mut(buddy_range.start)
                .expect("buddy not in range")
                .take()
                .expect("buddy shouldn't be empty");
            let bin = this
                .bins
                .get_mut(index)
                .expect("bin not in range")
                .take()
                .expect("bin should exist");
            let new = bin.merge(buddy);
            index = new.range().start;

            this.bins.set_unchecked(index, new);

            buddy_range = Self::buddy_of(new.range());
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
        if new_layout.size() > (DEPTH * MIN) {
            return Err(AllocError);
        }
        // fast path if the allocs are the same
        if new_layout.size() == old_layout.size() {
            return Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
                ptr.as_ptr(),
                new_layout.size(),
            )));
        }

        // FIXME: this is wrong on so many levels
        #[allow(unsafe_op_in_unsafe_fn, unused_unsafe)]
        let this = unsafe { &mut *(self as *const _ as usize as *mut Self) };

        let range = match this.bin_for(ptr) {
            Some(bin) if old_layout.size() <= bin.size() => bin.range(),
            _ => panic!("pointer is at an invalid bin or doesn't belong to this allocator"),
        };
        let index = range.start;

        let buddy_range = Self::buddy_of(range);
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
        if Self::is_right(buddy_range) && !this.is_used(buddy_index) {
            let buddy = this
                .bins
                .get_mut(buddy_index)
                .expect("buddy not in range")
                .take()
                .expect("buddy shouldn't be empty");
            let bin = this
                .bins
                .get_mut(index)
                .expect("bin not in range")
                .take()
                .expect("bin should exist");
            let new = bin.merge(buddy);
            this.bins.set_unchecked(index, new);

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

        // FIXME: this is wrong on so many levels
        #[allow(unsafe_op_in_unsafe_fn, unused_unsafe)]
        let this = unsafe { &mut *(self as *const _ as usize as *mut Self) };

        let range = match this.bin_for(ptr) {
            Some(bin) if old_layout.size() <= bin.size() => bin.range(),
            _ => panic!("pointer is at an invalid bin or doesn't belong to this allocator"),
        };

        let mut bin = this.bins.get(range.start).ok_or(AllocError)?.clone();

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
            bin = this.bins.get(start).ok_or(AllocError)?.clone();
        }

        this.mark_used(bin.range().start);

        Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
            this.addr_for(bin.range()),
            bin.size_bytes(),
        )))
    }
}

#[inline]
fn slot_range<const MIN: usize>(size: usize) -> core::iter::StepBy<Range<usize>> {
    (0..DEPTH).step_by(size / MIN)
}

impl<const M: usize> core::fmt::Debug for BuddyAllocator<'_, M> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BuddyAllocator")
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
        static mut BINS: [Bucket<128>; DEPTH] = [Bucket::empty(); DEPTH];

        let buddy = kcore::sync::SpinMutex::new(
            BuddyAllocator::<128>::new(
                unsafe { &mut BINS[..] },
                PageRangeInclusive::with_size(
                    VirtualAddr::new(0x0000_dead_beaf_0000),
                    DEPTH as u64 * 128,
                ),
            )
            .unwrap(),
        );

        for _ in 0..DEPTH {
            buddy.allocate(Layout::new::<u8>()).unwrap();
        }

        for bin in buddy.lock().bins.get_range(8..24) {
            assert!(!bin.is_empty());
            assert_eq!(bin.size(), 128)
        }

        assert!(buddy.lock().len() == DEPTH);
    }

    #[test]
    fn allocate_all_big() {
        static mut BINS: [Bucket<128>; DEPTH] = [Bucket::empty(); DEPTH];

        let buddy = kcore::sync::SpinMutex::new(
            BuddyAllocator::<128>::new(
                unsafe { &mut BINS[..] },
                PageRangeInclusive::with_size(
                    VirtualAddr::new(0x0000_dead_beaf_0000),
                    DEPTH as u64 * 128,
                ),
            )
            .unwrap(),
        );

        let _ = buddy.allocate(Layout::new::<[u8; 512]>()).unwrap();

        assert!(buddy.lock().len() == 1);
    }

    #[test]
    fn shrink_once() {
        // TODO: expand tests

        static mut BINS: [Bucket<128>; DEPTH] = [Bucket::empty(); DEPTH];

        let buddy = kcore::sync::SpinMutex::new(
            BuddyAllocator::<128>::new(
                unsafe { &mut BINS[..] },
                PageRangeInclusive::with_size(
                    VirtualAddr::new(0x0000_dead_beaf_0000),
                    DEPTH as u64 * 128,
                ),
            )
            .unwrap(),
        );

        let old_ptr = buddy.allocate(Layout::new::<[u8; 512]>()).unwrap();

        assert!(buddy.lock().len() == 1);
        assert!(buddy.lock().is_used(0));
        assert_eq!(buddy.lock().bins.get(0).map(Bucket::size), Some(4096));

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

        assert_eq!(buddy.lock().bins.get(0).map(Bucket::size), Some(2048));

        assert_eq!(buddy.lock().bins.get(16).map(Bucket::size), Some(2048));
    }

    #[test]
    fn grow_big_fast() {
        static mut BINS: [Bucket<128>; DEPTH] = [Bucket::empty(); DEPTH];

        let buddy = kcore::sync::SpinMutex::new(
            BuddyAllocator::<128>::new(
                unsafe { &mut BINS[..] },
                PageRangeInclusive::with_size(
                    VirtualAddr::new(0x0000_dead_beaf_0000),
                    DEPTH as u64 * 128,
                ),
            )
            .unwrap(),
        );

        // this should split the allocator in 3 pieces (2*1024 bits bins, 1*2048 bits bin)
        let ptr = buddy.allocate(Layout::new::<[u8; 128]>()).unwrap();
        assert_eq!(buddy.lock().bins.get(0).map(Bucket::size), Some(1024));
        assert_eq!(buddy.lock().bins.get(8).map(Bucket::size), Some(1024));
        assert_eq!(buddy.lock().bins.get(16).map(Bucket::size), Some(2048));

        unsafe {
            // this should merge the allocator in 2 pieces (2*2048 bits bins)
            let ptr = buddy
                .grow(
                    ptr.cast(),
                    Layout::new::<[u8; 128]>(),
                    Layout::new::<[u8; 256]>(),
                )
                .unwrap();

            assert_eq!(buddy.lock().bins.get(0).map(Bucket::size), Some(2048));
            assert_eq!(buddy.lock().bins.get(16).map(Bucket::size), Some(2048));

            // this should merge the allocator in 1 pieces (1*4096 bits bins)
            let _ = buddy
                .grow(
                    ptr.cast(),
                    Layout::new::<[u8; 256]>(),
                    Layout::new::<[u8; 512]>(),
                )
                .unwrap();

            assert_eq!(buddy.lock().bins.get(0).map(Bucket::size), Some(4096));
        }

        assert!(buddy.lock().len() == 1);
    }

    #[test]
    fn allocate_mixed() {
        static mut BINS: [Bucket<128>; DEPTH] = [Bucket::empty(); DEPTH];

        let buddy = kcore::sync::SpinMutex::new(
            BuddyAllocator::<128>::new(
                unsafe { &mut BINS[..] },
                PageRangeInclusive::with_size(
                    VirtualAddr::new(0x0000_dead_beaf_0000),
                    DEPTH as u64 * 128,
                ),
            )
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

        for bin in buddy.lock().bins.get_range(8..24) {
            assert!(!bin.is_empty());
            assert_eq!(bin.size(), 128);
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
        static mut BINS: [Bucket<128>; DEPTH] = [Bucket::empty(); DEPTH];

        let buddy = kcore::sync::SpinMutex::new(
            BuddyAllocator::<128>::new(
                unsafe { &mut BINS[..] },
                PageRangeInclusive::with_size(
                    VirtualAddr::new(0x0000_dead_beaf_0000),
                    DEPTH as u64 * 128,
                ),
            )
            .unwrap(),
        );

        let allocs: std::vec::Vec<_> = (0..DEPTH)
            .map(|_| buddy.allocate(Layout::new::<u8>()).unwrap())
            .collect();

        assert_eq!(allocs.len(), DEPTH);

        for alloc in allocs {
            unsafe {
                buddy.deallocate(alloc.cast(), Layout::new::<u8>());
            }
        }
        assert_eq!(
            buddy.lock().bins.get(0).map(Bucket::size),
            Some(DEPTH * 128)
        );
        for bin in buddy.lock().bins.get_range(1..) {
            assert!(bin.is_empty());
        }

        assert!(buddy.lock().len() == 0);
    }

    #[test]
    fn buddy_of_test() {
        assert_eq!(BuddyAllocator::<128>::buddy_of(0..1), 1..2);
        assert_eq!(BuddyAllocator::<128>::buddy_of(1..2), 0..1);

        assert_eq!(BuddyAllocator::<128>::buddy_of(2..3), 3..4);
        assert_eq!(BuddyAllocator::<128>::buddy_of(3..4), 2..3);

        assert_eq!(BuddyAllocator::<128>::buddy_of(4..5), 5..6);
        assert_eq!(BuddyAllocator::<128>::buddy_of(5..6), 4..5);

        assert_eq!(BuddyAllocator::<128>::buddy_of(0..2), 2..4);
        assert_eq!(BuddyAllocator::<128>::buddy_of(2..4), 0..2);

        assert_eq!(BuddyAllocator::<128>::buddy_of(0..4), 4..8);
        assert_eq!(BuddyAllocator::<128>::buddy_of(4..8), 0..4);

        assert_eq!(BuddyAllocator::<128>::buddy_of(0..8), 8..16);
        assert_eq!(BuddyAllocator::<128>::buddy_of(8..16), 0..8);

        assert_eq!(BuddyAllocator::<128>::buddy_of(0..16), 16..32);
        assert_eq!(BuddyAllocator::<128>::buddy_of(16..32), 0..16);

        assert_eq!(BuddyAllocator::<128>::buddy_of(0..32), 32..64);
        assert_eq!(BuddyAllocator::<128>::buddy_of(32..64), 0..32);
    }
}
