use alloc::alloc::{AllocError, Layout};
use core::{ops::Range, ptr::NonNull};

use libx64::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{frame::FrameRange, Page4Kb},
};

use crate::{kalloc::AllocatorMutImpl, AllocatorBin, AllocatorBinFlags};

const DEPTH: usize = 2usize.pow(6); // 2^6 = 64 bins

type MaskInt = u64;
type BucketInt = usize;

const fn buddy_of(range: Range<usize>) -> Range<usize> {
    let diff = range.end - range.start;

    if (range.start / diff) % 2 == 0 {
        range.end..(range.end + diff)
    } else {
        (range.start - diff)..range.start
    }
}

#[derive(Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct Bucket<const MIN: usize, T>(T);

const BIN_ALLOCATED: AllocatorBinFlags = AllocatorBinFlags::USR_BIT1;

impl<const MIN: usize> Bucket<MIN, AllocatorBin> {
    const fn new(start: usize, end: usize) -> Self {
        Self(AllocatorBin {
            flags: AllocatorBinFlags::USED,
            start: VirtualAddr::new(start as u64),
            end: VirtualAddr::new(end as u64),
            data: 0,
        })
    }
    pub const fn empty() -> Self {
        Self(AllocatorBin::with_flags(AllocatorBinFlags::USED))
    }

    pub fn into_inner(self) -> AllocatorBin {
        self.0
    }

    unsafe fn merge(self, rhs: Self) -> Self {
        let start = core::cmp::min(self.0.start.as_u64(), rhs.0.start.as_u64()) as usize;
        let end = core::cmp::max(self.0.end.as_u64(), rhs.0.end.as_u64()) as usize;
        Self::new(start, end)
    }

    fn split(self) -> (Bucket<MIN, AllocatorBin>, Bucket<MIN, AllocatorBin>) {
        let (start, end) = (
            Bucket::<MIN, _>::from(&self.0).start(),
            Bucket::<MIN, _>::from(&self.0).end(),
        );
        (
            Bucket::<MIN, AllocatorBin>::new(start, start + (end - start) / 2),
            Bucket::<MIN, AllocatorBin>::new(start + (end - start) / 2, end),
        )
    }
}

impl<const MIN: usize, T> Bucket<MIN, T>
where
    T: core::borrow::Borrow<AllocatorBin>,
{
    fn is_empty(&self) -> bool {
        self.start() == 0 && self.end() == 0 && self.0.borrow().data == 0
    }

    fn is_right(&self) -> bool {
        let range = self.range();
        (range.start / (range.end - range.start)) % 2 != 0
    }

    pub fn is_allocated(&self) -> bool {
        self.0.borrow().flags.contains(BIN_ALLOCATED)
    }

    fn start(&self) -> usize {
        self.0.borrow().start.as_u64() as usize
    }
    fn end(&self) -> usize {
        self.0.borrow().end.as_u64() as usize
    }

    fn range(&self) -> Range<usize> {
        self.start()..self.end()
    }

    fn size(&self) -> usize {
        (self.end() - self.start()) * MIN
    }

    fn size_bytes(&self) -> usize {
        self.size() / 8
    }

    fn to_owned(&self) -> Bucket<MIN, AllocatorBin> {
        Bucket::from(self.0.borrow().clone())
    }
}

impl<const MIN: usize> Bucket<MIN, &mut AllocatorBin> {
    pub fn mark_allocated(&mut self) {
        self.0.flags.insert(BIN_ALLOCATED);
    }

    pub fn mark_deallocated(&mut self) {
        self.0.flags.remove(BIN_ALLOCATED);
    }

    fn take_owned(&mut self) -> Option<Bucket<MIN, AllocatorBin>> {
        if Bucket::<MIN, _>::from(&*self.0).is_empty() {
            None
        } else {
            let ret = self.0.clone();
            *self.0 = Bucket::<MIN, AllocatorBin>::empty().into_inner();
            Some(Bucket::from(ret))
        }
    }
}

impl<'a, const MIN: usize> From<AllocatorBin> for Bucket<MIN, AllocatorBin> {
    fn from(bin: AllocatorBin) -> Self {
        Self(bin)
    }
}

impl<'a, const MIN: usize> From<&'a AllocatorBin> for Bucket<MIN, &'a AllocatorBin> {
    fn from(bin: &'a AllocatorBin) -> Self {
        Self(bin)
    }
}

impl<'a, const MIN: usize> From<&'a mut AllocatorBin> for Bucket<MIN, &'a mut AllocatorBin> {
    fn from(bin: &'a mut AllocatorBin) -> Self {
        Self(bin)
    }
}

impl<const MIN: usize, T> core::fmt::Debug for Bucket<MIN, T>
where
    T: core::borrow::Borrow<AllocatorBin>,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bucket")
            .field("size", &self.size())
            .field("range", &self.range())
            .finish()
    }
}

#[repr(transparent)]
pub struct BinSlice<'a, const MIN: usize> {
    bins: &'a mut [AllocatorBin],
}

impl<'a, const MIN: usize> core::fmt::Debug for BinSlice<'a, MIN> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.get_range(..)).finish()
    }
}

impl<'a, const MIN: usize> BinSlice<'a, MIN> {
    unsafe fn set_unchecked(
        &mut self,
        index: usize,
        bucket: Bucket<MIN, AllocatorBin>,
    ) -> Bucket<MIN, &mut AllocatorBin> {
        self.bins[index] = bucket.into_inner();
        Bucket::from(&mut self.bins[index])
    }

    fn get(&self, index: usize) -> Option<Bucket<MIN, &AllocatorBin>> {
        let bin = Bucket::from(self.bins.get(index)?);
        if bin.is_empty() {
            None
        } else {
            Some(bin)
        }
    }

    fn get_mut(&mut self, index: usize) -> Option<Bucket<MIN, &mut AllocatorBin>> {
        let bin = Bucket::from(self.bins.get_mut(index)?);
        if bin.is_empty() {
            None
        } else {
            Some(bin)
        }
    }

    fn get_deallocated_mut(&mut self, index: usize) -> Option<Bucket<MIN, &mut AllocatorBin>> {
        let bin = Bucket::from(self.bins.get_mut(index)?);
        if !bin.is_allocated() {
            Some(bin)
        } else {
            None
        }
    }

    fn available_for(&self, idx: usize, layout: Layout) -> bool {
        matches!(self.get(idx), Some(bin) if layout.size() <= bin.size_bytes() && !bin.is_allocated())
    }

    fn iter_available(&self, layout: Layout) -> impl Iterator<Item = usize> + '_ {
        let size = core::cmp::max(layout.size().next_power_of_two() * 8, MIN as usize);
        (0..DEPTH)
            .step_by(size / MIN)
            .filter(move |&i| self.available_for(i, layout))
    }

    fn get_range<U>(&self, range: U) -> impl Iterator<Item = Bucket<MIN, &AllocatorBin>>
    where
        U: core::slice::SliceIndex<[AllocatorBin], Output = [AllocatorBin]>,
    {
        self.bins[range].iter().map(Bucket::from)
    }
}

#[repr(C)]
pub struct BuddyAllocator<'a, const MIN: usize> {
    bins: BinSlice<'a, MIN>,
    start: PhysicalAddr,
    used_mask: MaskInt,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Error {
    InvalidPowerOfTwo,
    InvalidBinSliceSize,
    InvalidFrameRange { expected: usize, got: usize },
}

impl<'a, const MIN: usize> BuddyAllocator<'a, MIN> {
    pub fn new(bins: &'a mut [AllocatorBin], page: FrameRange<Page4Kb>) -> Result<Self, Error> {
        if !MIN.is_power_of_two() {
            return Err(Error::InvalidPowerOfTwo);
        }
        if bins.len() != DEPTH {
            return Err(Error::InvalidBinSliceSize);
        }
        let expected = (MIN * DEPTH) / (Page4Kb as usize);
        let got = page.len();

        if expected != got {
            return Err(Error::InvalidFrameRange { expected, got });
        }

        let start = page.start();

        bins.iter_mut().for_each(|bin| {
            bin.flags.insert(AllocatorBinFlags::USED);
        });
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

    pub fn contains(&self, ptr: *const u8) -> bool {
        ((self.start.as_u64() as usize)..(self.start.as_u64() as usize + DEPTH * MIN))
            .contains(&(ptr as usize))
    }

    pub const fn pages(&self) -> usize {
        DEPTH * MIN / (Page4Kb as usize)
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
            // SAFETY: it should be the case, and is checked in debug mode
            unsafe { core::intrinsics::assume(size.is_power_of_two()) };

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

    fn mark_used(&mut self, idx: usize) {
        self.bins.get_mut(idx).unwrap().mark_allocated();
        self.used_mask |= 1 << idx;
    }
    fn mark_unused(&mut self, idx: usize) {
        self.bins.get_mut(idx).unwrap().mark_deallocated();
        self.used_mask &= !(1 << idx);
    }

    fn split_at(&mut self, idx: usize) -> Option<(usize, usize)> {
        let used = self.is_used(idx);
        let bin = self.bins.get_mut(idx).and_then(|mut bin| {
            if used || bin.size() <= MIN {
                return None;
            } else {
                bin.take_owned()
            }
        })?;

        let (left, right) = bin.split();
        let (lstart, rstart) = (left.start(), right.start());

        // SAFETY: TODO
        unsafe {
            self.bins.set_unchecked(lstart, left);
            self.bins.set_unchecked(rstart, right);
        }
        Some((lstart, rstart))
    }

    const unsafe fn addr_for(&self, range: Range<usize>) -> *mut u8 {
        let start = range.start;
        (self.start.as_u64() + (start * MIN) as u64) as *mut u8
    }

    fn bin_for(&self, ptr: NonNull<u8>) -> Option<Bucket<MIN, &AllocatorBin>> {
        let idx = (ptr.as_ptr() as u64 - self.start.as_u64()) as usize / MIN;
        self.bins.get(idx)
    }

    fn buddy_of_bucket<'b, T>(
        &'b mut self,
        bin: &Bucket<MIN, T>,
    ) -> Option<Bucket<MIN, &'b mut AllocatorBin>>
    where
        T: core::borrow::Borrow<AllocatorBin>,
    {
        //  A buddy bucket is valid if:
        let buddy_range = buddy_of(bin.range());

        //  1. It is in bin range (< 64)
        (buddy_range.start < DEPTH).then_some(())?;

        //  2. Buddy is not allocated
        (!self.is_used(buddy_range.start)).then_some(())?;
        let buddy = self.bins.get_deallocated_mut(buddy_range.start)?;

        //  3. Buckets have the same size (bin.size() == buddy.size())
        (bin.size() == buddy.size()).then_some(())?;

        Some(buddy)
    }

    fn allocate_split(
        &mut self,
        mut bin: Bucket<MIN, AllocatorBin>,
        layout: Layout,
    ) -> Result<Bucket<MIN, AllocatorBin>, AllocError> {
        let size = core::cmp::max(layout.size().next_power_of_two() * 8, MIN as usize);

        while size <= bin.size() / 2 {
            let split = bin.start();
            let (start, _) = self.split_at(split).ok_or(AllocError)?;

            bin = self.bins.get(start).ok_or(AllocError)?.to_owned();
        }
        Ok(bin)
    }

    fn deallocate_merge(
        &mut self,
        bin: Bucket<MIN, AllocatorBin>,
    ) -> Option<Bucket<MIN, AllocatorBin>> {
        let range = bin.range();
        let mut index = range.start;

        self.mark_unused(index);

        // Merge the current bin with its buddy if possible.
        loop {
            let bin = self
                .bins
                .get_mut(index)
                .as_mut()
                .map(Bucket::take_owned)
                .flatten()?;

            let buddy = match self
                .buddy_of_bucket(&bin)
                .as_mut()
                .map(Bucket::take_owned)
                .flatten()
            {
                Some(buddy) => buddy,
                None => {
                    // put back in place, it is marked as unused so it will be picked up by the
                    // next dealocate call. Return the last deallocated bin (could be used to
                    // recycle it right away as we merge left)
                    unsafe { self.bins.set_unchecked(index, bin.to_owned()) };
                    return Some(bin);
                }
            };

            let new = unsafe { bin.merge(buddy) };
            index = unsafe { self.bins.set_unchecked(new.start(), new) }.start();
        }
    }
}

unsafe impl<const MIN: usize> AllocatorMutImpl for BuddyAllocator<'_, MIN> {
    fn allocate_mut(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let size = core::cmp::max(layout.size().next_power_of_two() * 8, MIN as usize);

        // fast path
        if !self.have_buckets_for(size) {
            return Err(AllocError);
        }

        let free_index = self.bins.iter_available(layout).next();
        if let Some(i) = free_index {
            let mut bin = self.bins.get(i).ok_or(AllocError)?.to_owned();
            bin = self.allocate_split(bin, layout)?;

            let range = bin.range();
            let size = bin.size_bytes();

            self.mark_used(i);

            return Ok(unsafe {
                NonNull::new_unchecked(core::slice::from_raw_parts_mut(self.addr_for(range), size))
            });
        } else {
            Err(AllocError)
        }
    }

    unsafe fn deallocate_mut(&mut self, ptr: NonNull<u8>, layout: Layout) {
        let bin = match self.bin_for(ptr) {
            Some(bin) if layout.size() <= bin.size() => bin.to_owned(),
            _ => panic!("pointer is at an invalid bin or doesn't belong to this allocator"),
        };
        self.deallocate_merge(bin);
    }

    unsafe fn grow_mut(
        &mut self,
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

        let bin = match self.bin_for(ptr) {
            Some(bin) if old_layout.size() <= bin.size() => bin.to_owned(),
            _ => panic!("pointer is at an invalid bin or doesn't belong to this allocator"),
        };

        // Fast path:
        //
        // Merge the two blocks without copying anything
        //
        // 1.
        //             |
        //          (Parent)        <- this bin index is the same as Bin
        //          /       \
        //      Bin         Buddy(unused)
        // 2.
        //             |
        //     Parent = Bin + Buddy
        //
        // NOTE: Only checks one tree level where it recurse upwards
        let buddy = self
            .buddy_of_bucket(&bin)
            .as_mut()
            .map(Bucket::take_owned)
            .flatten();
        match buddy {
            Some(buddy) if buddy.is_right() && bin.size_bytes() * 2 >= new_layout.size() => {
                let index = bin.start();
                let bin = self
                    .bins
                    .get_mut(index)
                    .expect("bin not in range")
                    .take_owned()
                    .expect("bin should exist");
                let new = bin.merge(buddy);
                self.bins.set_unchecked(index, new);

                return Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
                    ptr.as_ptr(),
                    new_layout.size(),
                )));
            }
            _ => {}
        }

        let new_ptr = self.allocate_mut(new_layout)?;

        // SAFETY: because `new_layout.size()` must be greater than or equal to
        // `old_layout.size()`, both the old and new memory allocation are valid for reads and
        // writes for `old_layout.size()` bytes. Also, because the old allocation wasn't yet
        // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
        // safe. The safety contract for `dealloc` must be upheld by the caller.
        core::ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), old_layout.size());
        self.deallocate_mut(ptr, old_layout);

        Ok(new_ptr)
    }

    unsafe fn grow_zeroed_mut(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );
        let new_ptr = self.grow_mut(ptr, old_layout, new_layout)?;
        new_ptr
            .as_non_null_ptr()
            .as_ptr()
            .add(old_layout.size())
            .write_bytes(0, new_layout.size() - old_layout.size());

        Ok(new_ptr)
    }

    unsafe fn shrink_mut(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );

        let mut bin = self
            .bin_for(ptr)
            .and_then(|bin| (old_layout.size() <= bin.size()).then_some(bin))
            .ok_or(AllocError)?
            .to_owned();

        // forward the pointer since we can't shrink (no one has too much memory I guess)
        if bin.size() == MIN {
            return Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
                ptr.as_ptr(),
                bin.size_bytes(),
            )));
        }

        self.mark_unused(bin.start());

        bin = self.allocate_split(bin, new_layout)?;

        let range = bin.range();
        let size = bin.size_bytes();
        self.mark_used(range.start);

        Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
            self.addr_for(range),
            size,
        )))
    }
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

    const ADDR: PhysicalAddr = PhysicalAddr::new(0xdead_beef);

    #[test]
    fn allocate_all_slabs() {
        static mut BINS: [AllocatorBin; DEPTH] = [AllocatorBin::new(); DEPTH];

        let mut buddy = BuddyAllocator::<128>::new(
            unsafe { &mut BINS[..] },
            FrameRange::with_size(ADDR, DEPTH as u64 * 128),
        )
        .unwrap();

        for _ in 0..DEPTH {
            buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        }

        for bin in buddy.bins.get_range(8..24) {
            assert!(!bin.is_empty());
            assert_eq!(bin.size(), 128)
        }

        assert!(buddy.len() == DEPTH);
    }

    #[test]
    fn allocate_all_big() {
        static mut BINS: [AllocatorBin; DEPTH] = [AllocatorBin::new(); DEPTH];

        let mut buddy = BuddyAllocator::<128>::new(
            unsafe { &mut BINS[..] },
            FrameRange::with_size(ADDR, DEPTH as u64 * 128),
        )
        .unwrap();

        let _ = buddy.allocate_mut(Layout::new::<[u8; 512]>()).unwrap();

        assert!(buddy.len() == 1);
    }

    #[test]
    fn shrink_once() {
        // TODO: expand tests
        static mut BINS: [AllocatorBin; DEPTH] = [AllocatorBin::new(); DEPTH];

        let mut buddy = BuddyAllocator::<128>::new(
            unsafe { &mut BINS[..] },
            FrameRange::with_size(ADDR, DEPTH as u64 * 128),
        )
        .unwrap();

        let old_ptr = buddy.allocate_mut(Layout::new::<[u8; 512]>()).unwrap();

        assert!(buddy.len() == 1);
        assert!(buddy.is_used(0));
        assert_eq!(buddy.bins.get(0).as_ref().map(Bucket::size), Some(4096));

        let new_ptr = unsafe {
            buddy
                .shrink_mut(
                    old_ptr.cast(),
                    Layout::new::<[u8; 512]>(),
                    Layout::new::<[u8; 256]>(),
                )
                .unwrap()
        };

        assert!(buddy.len() == 1);

        assert!(buddy.is_used(0));
        assert!(!buddy.is_used(16));

        // SAFETY: we never read anything else than the pointer value
        unsafe { assert_eq!(new_ptr.as_ref().as_ptr(), old_ptr.as_ref().as_ptr()) };

        assert_eq!(buddy.bins.get(0).as_ref().map(Bucket::size), Some(2048));

        assert_eq!(buddy.bins.get(16).as_ref().map(Bucket::size), Some(2048));
    }

    #[test]
    fn grow_big_fast() {
        static mut BINS: [AllocatorBin; DEPTH] = [AllocatorBin::new(); DEPTH];

        let mut buddy = BuddyAllocator::<128>::new(
            unsafe { &mut BINS[..] },
            FrameRange::with_size(ADDR, DEPTH as u64 * 128),
        )
        .unwrap();

        // this should split the allocator in 3 pieces (2*1024 bits bins, 1*2048 bits bin)
        let ptr = buddy.allocate_mut(Layout::new::<[u8; 128]>()).unwrap();
        assert_eq!(buddy.bins.get(0).as_ref().map(Bucket::size), Some(1024));
        assert!(buddy
            .bins
            .get(0)
            .as_ref()
            .map(Bucket::is_allocated)
            .unwrap_or(false));
        assert_eq!(buddy.bins.get(8).as_ref().map(Bucket::size), Some(1024));
        assert!(!buddy
            .bins
            .get(8)
            .as_ref()
            .map(Bucket::is_allocated)
            .unwrap_or(true));
        assert_eq!(buddy.bins.get(16).as_ref().map(Bucket::size), Some(2048));
        assert!(!buddy
            .bins
            .get(16)
            .as_ref()
            .map(Bucket::is_allocated)
            .unwrap_or(true));

        unsafe {
            // this should merge the allocator in 2 pieces (2*2048 bits bins)
            let ptr = buddy
                .grow_mut(
                    ptr.cast(),
                    Layout::new::<[u8; 128]>(),
                    Layout::new::<[u8; 256]>(),
                )
                .unwrap();

            assert_eq!(buddy.bins.get(0).as_ref().map(Bucket::size), Some(2048));
            assert_eq!(buddy.bins.get(16).as_ref().map(Bucket::size), Some(2048));

            // this should merge the allocator in 1 pieces (1*4096 bits bins)
            let _ = buddy
                .grow_mut(
                    ptr.cast(),
                    Layout::new::<[u8; 256]>(),
                    Layout::new::<[u8; 512]>(),
                )
                .unwrap();

            assert_eq!(buddy.bins.get(0).as_ref().map(Bucket::size), Some(4096));
        }

        assert!(buddy.len() == 1);
    }

    #[test]
    fn allocate_mixed() {
        static mut BINS: [AllocatorBin; DEPTH] = [AllocatorBin::new(); DEPTH];

        let mut buddy = BuddyAllocator::<128>::new(
            unsafe { &mut BINS[..] },
            FrameRange::with_size(ADDR, DEPTH as u64 * 128),
        )
        .unwrap();

        // 2 - 512 bits slot = 1024 bits
        let _ = buddy.allocate_mut(Layout::new::<[u8; 64]>()).unwrap();
        assert!(buddy.is_used(0));
        assert!(!buddy.is_used(1));
        assert!(!buddy.is_used(2));
        assert!(!buddy.is_used(3));
        assert!(!buddy.is_used(4));

        let _ = buddy.allocate_mut(Layout::new::<[u8; 64]>()).unwrap();
        assert!(buddy.is_used(4));
        assert!(!buddy.is_used(5));
        assert!(!buddy.is_used(6));
        assert!(!buddy.is_used(7));

        // 16 - 128 bits slot = 2048 bits
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();

        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();
        let _ = buddy.allocate_mut(Layout::new::<u8>()).unwrap();

        for bin in buddy.bins.get_range(8..24) {
            assert!(!bin.is_empty());
            assert_eq!(bin.size(), 128);
        }

        // 1 - 1024 bits slot
        let _ = buddy.allocate_mut(Layout::new::<[u8; 128]>()).unwrap();
        assert!(buddy.is_used(24));
        assert!(!buddy.is_used(25));
        assert!(!buddy.is_used(26));
        assert!(!buddy.is_used(27));
        assert!(!buddy.is_used(28));
        assert!(!buddy.is_used(29));
        assert!(!buddy.is_used(30));
        assert!(!buddy.is_used(31));

        assert!(buddy.len() == 19);
    }

    #[test]
    fn deallocate_all_slabs() {
        static mut BINS: [AllocatorBin; DEPTH] = [AllocatorBin::new(); DEPTH];

        let mut buddy = BuddyAllocator::<128>::new(
            unsafe { &mut BINS[..] },
            FrameRange::with_size(ADDR, DEPTH as u64 * 128),
        )
        .unwrap();

        let allocs: std::vec::Vec<_> = (0..DEPTH)
            .map(|_| buddy.allocate_mut(Layout::new::<u8>()).unwrap())
            .collect();

        assert_eq!(allocs.len(), DEPTH);

        for alloc in allocs {
            unsafe {
                buddy.deallocate_mut(alloc.cast(), Layout::new::<u8>());
            }
        }
        assert_eq!(
            buddy.bins.get(0).as_ref().map(Bucket::size),
            Some(DEPTH * 128)
        );
        for bin in buddy.bins.get_range(1..) {
            assert!(bin.is_empty());
        }

        assert!(buddy.len() == 0);
    }

    #[test]
    fn buddy_of_test() {
        assert_eq!(buddy_of(0..1), 1..2);
        assert_eq!(buddy_of(1..2), 0..1);

        assert_eq!(buddy_of(2..3), 3..4);
        assert_eq!(buddy_of(3..4), 2..3);

        assert_eq!(buddy_of(4..5), 5..6);
        assert_eq!(buddy_of(5..6), 4..5);

        assert_eq!(buddy_of(0..2), 2..4);
        assert_eq!(buddy_of(2..4), 0..2);

        assert_eq!(buddy_of(0..4), 4..8);
        assert_eq!(buddy_of(4..8), 0..4);

        assert_eq!(buddy_of(0..8), 8..16);
        assert_eq!(buddy_of(8..16), 0..8);

        assert_eq!(buddy_of(0..16), 16..32);
        assert_eq!(buddy_of(16..32), 0..16);

        assert_eq!(buddy_of(0..32), 32..64);
        assert_eq!(buddy_of(32..64), 0..32);
    }
}
