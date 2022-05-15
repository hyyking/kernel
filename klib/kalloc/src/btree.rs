use alloc::boxed::Box;
use core::ptr::NonNull;

use crate::{AllocatorBin, AllocatorBinFlags};

use libx64::paging::page::PageRange;
use libx64::paging::Page4Kb;

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
struct BTreeAllocatorBin(NonNull<AllocatorBin>);

impl BTreeAllocatorBin {
    fn bin(&self) -> &AllocatorBin {
        unsafe { self.0.as_ref() }
    }
    fn bin_mut(&mut self) -> &mut AllocatorBin {
        unsafe { self.0.as_mut() }
    }
    fn elements(&self) -> Option<&BTreeElements> {
        Some(unsafe { self.0.as_ref().cast_data_ptr::<BTreeElements>()?.as_ref() })
    }

    fn elements_mut(&mut self) -> Option<&mut BTreeElements> {
        Some(unsafe { self.0.as_ref().cast_data_ptr::<BTreeElements>()?.as_mut() })
    }
}

impl core::fmt::Debug for BTreeAllocatorBin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut dbg = f.debug_struct("BTreeAllocatorBin");
        dbg.field("bin", self.bin());
        if let Some(elements) = self.elements() {
            dbg.field("children", &elements);
        }
        dbg.finish()
    }
}

pub struct BTreeElements {
    elements: [Option<BTreeAllocatorBin>; Self::ELEMENTS],
}

impl core::fmt::Debug for BTreeElements {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.elements).finish()
    }
}

impl BTreeElements {
    const ELEMENTS: usize = 32;

    pub const fn empty() -> Self {
        Self {
            elements: [None; Self::ELEMENTS],
        }
    }

    pub fn new<I>(range: PageRange<Page4Kb>, bins: &mut I) -> Box<Self>
    where
        I: Iterator<Item = &'static mut AllocatorBin>,
    {
        let mut range_len = range.end().as_u64() - range.start().as_u64();
        let bin_size = (range_len / u64::try_from(Self::ELEMENTS).unwrap()).next_power_of_two();

        let mut root = Box::new(Self::empty());

        let mut start = range.start();
        let mut i = 0;

        while range_len > 0 {
            let size = core::cmp::min(bin_size, range_len);
            let end = start + size;

            match bins.next() {
                Some(bin) => {
                    bin.flags |= AllocatorBinFlags::USED;
                    bin.start = start;
                    bin.end = end;
                    root.elements[i] = Some(BTreeAllocatorBin(NonNull::from(bin)));
                }
                None => panic!("not enought bins"),
            }
            i += 1;

            range_len -= size;
            start = end;
        }

        root
    }

    pub fn new_recursive<I>(range: PageRange<Page4Kb>, bins: &mut I) -> Box<Self>
    where
        I: Iterator<Item = &'static mut AllocatorBin> + 'static,
    {
        let mut this = Self::new(range.clone(), &mut *bins);
        for (_, bin) in this.bin_iter_mut().filter(|(_, b)| b.len() > 1) {
            let elements = BTreeElements::new_recursive(bin.range(), &mut *bins);
            bin.data = Box::leak(elements) as *mut _ as usize;
        }
        this
    }

    pub fn bin_iter(&self) -> impl Iterator<Item = (usize, &'_ AllocatorBin)> {
        self.elements
            .iter()
            .enumerate()
            .filter_map(|(i, element)| Some((i, element.as_ref()?.bin())))
    }

    pub fn bin_iter_mut(&mut self) -> impl Iterator<Item = (usize, &'_ mut AllocatorBin)> {
        self.elements
            .iter_mut()
            .enumerate()
            .filter_map(|(i, element)| Some((i, element.as_mut()?.bin_mut())))
    }

    pub fn children(&self) -> impl Iterator<Item = (usize, &'_ BTreeElements)> {
        self.elements
            .iter()
            .enumerate()
            .filter_map(|(i, bin)| Some((i, bin.as_ref()?.elements()?)))
    }

    pub fn children_mut(&mut self) -> impl Iterator<Item = (usize, &'_ mut BTreeElements)> {
        self.elements
            .iter_mut()
            .enumerate()
            .filter_map(|(i, bin)| Some((i, bin.as_mut()?.elements_mut()?)))
    }
}

#[derive(Debug)]
pub struct BTreeAllocator {
    pub range: PageRange<Page4Kb>,
    pub root: Box<BTreeElements>,
}

impl BTreeAllocator {
    pub fn new<I>(range: PageRange<Page4Kb>, mut bins: I) -> Self
    where
        I: Iterator<Item = &'static mut AllocatorBin> + 'static,
    {
        let root = BTreeElements::new_recursive(range.clone(), &mut bins);
        Self { range, root }
    }
}

#[cfg(test)]
mod tests {

    use libx64::address::VirtualAddr;

    use super::*;

    const N: usize = 32 + 32usize.pow(2) + 32usize.pow(3) + 32usize.pow(4);
    const MEM: usize = 32 * 32 * 32 * 32 * Page4Kb as usize;

    static mut BINS: [AllocatorBin; N] = [AllocatorBin::new(); N];

    const TEST_RANGE: PageRange<Page4Kb> =
        PageRange::new_addr(VirtualAddr::new(0), VirtualAddr::new(MEM as u64));

    #[test]
    fn it_works() {
        let _tree = BTreeAllocator::new(TEST_RANGE, unsafe { BINS.iter_mut() });

        let pre_alloc_sz = core_alloc::alloc::Layout::new::<[AllocatorBin; N]>().size() as f64;
        let mem = MEM as f64;

        dbg!(N);
        dbg!(mem);
        dbg!(mem / 4096.0);
        dbg!(core_alloc::alloc::Layout::new::<AllocatorBin>());
        dbg!(pre_alloc_sz * 8.0 / 4096.0);

        dbg!(("efficiency:", mem / (pre_alloc_sz * 8.0)));
    }
}
