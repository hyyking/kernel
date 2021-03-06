use core::{arch::asm, marker::PhantomData};

use crate::{
    address::VirtualAddr,
    descriptors::{GdtEntry, ToGdtEntry},
    segments::SegmentSelector,
    Privilege,
};

const GDT_ENTRIES: usize = 8;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct GlobalDescriptorTable {
    entries: [u64; GDT_ENTRIES],
    at: u16,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct GdtPtr<'a> {
    limit: u16,
    addr: VirtualAddr,
    _m: PhantomData<&'a ()>,
}

impl<'a> core::fmt::Debug for GdtPtr<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GdtPtr")
            .field("limit", &{ self.limit })
            .field("base", &{ self.addr })
            .finish()
    }
}

impl GlobalDescriptorTable {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [0_u64; GDT_ENTRIES],
            at: 0,
        }
    }

    /// Get a reference to the global descriptor table's entries.
    #[inline]
    #[must_use]
    pub fn entries(&self) -> &[u64] {
        &self.entries[..usize::from(self.at)]
    }

    #[inline]
    #[must_use]
    pub fn lgdt_ptr(&self) -> GdtPtr<'_> {
        GdtPtr {
            limit: self.at * (core::mem::size_of::<u64>() as u16) - 1,
            addr: VirtualAddr::from_ptr(self.entries().as_ptr()),
            _m: PhantomData,
        }
    }

    pub fn add_entry<T: ToGdtEntry + core::fmt::Debug>(&mut self, entry: T) -> SegmentSelector {
        tracing::trace!(entry = debug(&entry));
        let idx = match entry.to_gdt_entry() {
            GdtEntry::Null => self.push(0),
            GdtEntry::User(user) => unsafe {
                debug_assert_eq!(core::mem::size_of_val(&user), 8);
                self.push(core::mem::transmute::<_, u64>(user))
            },
            GdtEntry::Gate(gate) => unsafe {
                debug_assert_eq!(core::mem::size_of_val(&gate), 16);
                let bytes = core::mem::transmute::<_, u128>(gate);
                let idx = self.push(bytes as u64);
                self.push((bytes >> 64) as u64);
                idx
            },
            GdtEntry::System(ss) => unsafe {
                debug_assert_eq!(core::mem::size_of_val(&ss), 16);
                let bytes = core::mem::transmute::<_, u128>(ss);
                let idx = self.push(bytes as u64);
                self.push((bytes >> 64) as u64);
                idx
            },
        };
        SegmentSelector::zero()
            .set_index(idx)
            .set_rpl(u16::from(Privilege::Ring0))
    }
    fn push(&mut self, value: u64) -> u16 {
        let next = self.at;
        self.entries[usize::from(next)] = value;
        self.at += 1;
        next
    }
}

pub fn lgdt(gdt: &GdtPtr) {
    // SAFETY: we assure the GDT pointer is well defined
    unsafe {
        asm!("lgdt [{}]", in(reg) gdt, options(readonly, nostack, preserves_flags));
    }
}

#[inline]
#[must_use]
pub fn sgdt() -> GdtPtr<'static> {
    let mut gdt = GdtPtr {
        limit: 0,
        addr: VirtualAddr::new(0),
        _m: PhantomData,
    };
    unsafe {
        asm!("sgdt [{}]", in(reg) &mut gdt, options(nostack, preserves_flags));
    }
    gdt
}

impl core::fmt::Debug for GlobalDescriptorTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.debug_struct("Gdt")
            .field("entries", &self.entries())
            .finish()
    }
}
