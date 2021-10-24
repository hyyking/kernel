use crate::{
    address::PhysicalAddr,
    paging::{
        frame::{FrameError, PhysicalFrame},
        table::{Level1, Level2, Level3, Level4},
        NotGiantPageSize, NotHugePageSize, Page1Gb, Page2Mb, Page4Kb, PageCheck,
    },
};

#[derive(Debug, Clone)]
pub enum MappedPage {
    Page4Kb(PhysicalFrame<Page4Kb>),
    Page2Mb(PhysicalFrame<Page2Mb>),
    Page1Gb(PhysicalFrame<Page1Gb>),
}

pub enum MappedLevel2Page {
    Page4Kb(PhysicalFrame<Page4Kb>),
    Page2Mb(PhysicalFrame<Page2Mb>),
}

pub enum MappedLevel3Page {
    Page4Kb(PhysicalFrame<Page4Kb>),
    Page1Gb(PhysicalFrame<Page1Gb>),
}

impl MappedPage {
    pub fn is_huge(&self) -> bool {
        !matches!(self, Self::Page4Kb(..))
    }
}

bitflags::bitflags! {
    pub struct Flags: u64 {
        const PRESENT =     1 << 0;
        const RW =          1 << 1;
        const US =          1 << 2;
        const PWL =         1 << 3;
        const PCD =         1 << 4;
        const ACCESSED =    1 << 5;
        const DIRTY =       1 << 6;
        const HUGE =        1 << 7;
        const NX =          1 << 63;
    }
}

bitfield::bitfield! {
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct RawPageEntry: u64 {
        /// This bit indicates whether the page-translation table or physical page is loaded
        /// in physical memory. When the P bit is cleared to 0, the table or physical page is
        /// not loaded in physical memory.
        /// When the P bit is set to 1, the table or physical page is loaded in physical memory.
        pub present: 0..1,

        /// This bit controls read/write access to all physical pages mapped by the
        /// table entry. For example, a page-map level-4 R/W bit controls read/write
        /// access to all 128M (512 × 512 × 512) physical pages it maps through the
        /// lower-level translation tables.
        /// When the R/W bit is cleared to 0, access is restricted to read-only. When
        /// the R/W bit is set to 1, both read and write access is allowed.
        pub rw: 1..2,

        /// This bit controls user (CPL 3) access to all physical pages mapped
        /// by the table entry. For example, a page-map level-4 U/S bit controls the access allowed
        /// to all 128M (512 × 512 × 512) physical pages it maps through the lower-level
        /// translation tables. When the U/S bit is cleared to 0, access is restricted to
        /// supervisor level (CPL 0, 1, 2). When the U/S bit is set to 1, both user and supervisor
        /// access is allowed.
        pub us: 2..3,

        /// This bit indicates whether the page-translation table or
        /// physical page to which this entry points has a writeback or writethrough caching
        /// policy. When the PWT bit is cleared to 0, the table or physical page has a writeback
        /// caching policy.
        /// When the PWT bit is set to 1, the table or physical page has a writethrough caching
        /// policy.
        pub pwl: 3..4,

        /// This bit indicates whether the page-translation table or
        /// physical page to which this entry points is cacheable. When the PCD bit is cleared to
        /// 0, the table or physical page is cacheable. When the PCD bit is set to 1, the table or
        /// physical page is not cacheable.
        pub pcd: 4..5,

        /// This bit indicates whether the page-translation table or physical page to
        /// which this entry points has been accessed. The A bit is set to 1 by the processor the
        /// first time the table or physical page is either read from or written to. The A bit is
        /// never cleared by the processor. Instead, software must clear this bit to 0 when it
        /// needs to track the frequency of table or physical-page accesses.
        pub access: 5..6,

        /// This bit is only present in the lowest level of the page-translation hierarchy. It
        /// indicates whether the physical page to which this entry points has been written. The D
        /// bit is set to 1 by the processor the first time there is a write to the physical page.
        /// The D bit is never cleared by the processor. Instead, software must clear this bit to 0
        /// when it needs to track the frequency of physical-page writes.
        pub dirty: 6..7,


        /// This bit is present in page-directory entries and long-mode page-directory-
        /// pointer entries. When the PS bit is set in the page-directory-pointer entry (PDPE) or
        /// page-directory entry (PDE), that entry is the lowest level of the page-translation
        /// hierarchy. When the PS bit is cleared to 0 in all levels above PTE, the lowest level of
        /// the page-translation hierarchy is the page-table entry (PTE), and the physical-page
        /// size is 4 Kbytes. The physical-page size is determined as follows:
        ///
        /// - If EFER.LMA=1 and PDPE.PS=1, the physical-page size is 1 Gbyte.
        /// - If CR4.PAE=0 and PDE.PS=1, the physical-page size is 4 Mbytes.
        /// - If CR4.PAE=1 and PDE.PS=1, the physical-page size is 2 Mbytes.
        pub huge: 7..8,

        /// This bit is only present in the lowest level of the page-translation
        /// hierarchy. It indicates the physical page is a global page. The TLB entry for a global page
        /// (G=1) is not invalidated when CR3 is loaded either explicitly by a MOV CRn instruction
        /// or implicitly during a task switch. Use of the G bit requires the page-global enable
        /// bit in CR4 to be set to 1 (CR4.PGE=1).
        pub global_page: 8..9,

        /// These bits are not interpreted by the processor and are available for
        /// use by system software.
        avl: 9..10,

        /// This bit is only present in the lowest level of the page-translation
        /// hierarchy, as follows:
        ///
        /// - If the lowest level is a PTE (PDE.PS=0), PAT occupies bit 7.
        /// - If the lowest level is a PDE (PDE.PS=1) or PDPE (PDPE.PS=1), PAT occupies bit 12.
        ///
        /// The PAT bit is the high-order bit of a 3-bit index into the PAT register (Figure 7-10
        /// on page 216). The other two bits involved in forming the index are the PCD and PWT
        /// bits. Not all processors support the PAT bit by implementing the PAT registers.
        pub pat: 10..11,

        pub user_bits: 52..59,

        /// When Memory Protection Keys are enabled (CR4.PKE=1), this 4-bit field selects the
        /// memory protection key for the physical page mapped by this entry. Ignored if memory
        /// protection keys are disabled (CR4.PKE=0).
        pub mpk: 59..63,

        /// This bit controls the ability to execute code from all physical pages mapped by the table
        /// entry. For example, a page-map level-4 NX bit controls the ability to execute code from
        /// all 128M (512 × 512 × 512) physical pages it maps through the lower-level translation
        /// tables. When the NX bit is cleared to 0, code can be executed from the mapped physical
        /// pages. When the NX bit is set to 1, code cannot be executed from the mapped physical
        /// pages.
        pub nx: 63..64,
    }
}

impl RawPageEntry {
    pub const fn set_address(self, addr: PhysicalAddr) -> Self {
        Self(self.as_u64() | addr.as_u64())
    }

    pub const fn set_flags(self, flags: Flags) -> Self {
        Self(self.as_u64() | flags.bits())
    }

    pub const fn get_flags(self) -> Flags {
        Flags::from_bits_truncate(self.as_u64())
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageEntry<L> {
    raw: RawPageEntry,
    _level: core::marker::PhantomData<L>,
}

impl<L> PageEntry<L> {
    pub const fn address(&self) -> PhysicalAddr {
        PhysicalAddr::new(self.raw.0 & 0x000F_FFFF_FFFF_F000)
    }

    pub const fn raw(&self) -> &RawPageEntry {
        &self.raw
    }

    pub fn set_flags(&mut self, flags: Flags) {
        self.raw = self.raw.set_flags(flags);
    }

    pub const fn get_flags(&self) -> Flags {
        self.raw.get_flags()
    }

    pub const fn is_huge(&self) -> bool {
        self.raw.get_flags().contains(Flags::HUGE)
    }

    pub const fn is_present(&self) -> bool {
        self.raw.get_flags().contains(Flags::PRESENT)
    }

    pub fn get_mpk(&self) -> u64 {
        self.raw.get_mpk()
    }

    pub fn set_mpk(&mut self, val: u64) {
        self.raw = self.raw.set_mpk(val);
    }
}

impl PageEntry<Level1> {
    pub const fn set_frame(self, addr: PhysicalFrame<Page4Kb>) -> Self {
        PageEntry {
            raw: self.raw.set_address(addr.ptr()),
            _level: core::marker::PhantomData,
        }
    }

    pub const fn frame(&self) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        if self.is_present() {
            Err(FrameError::EntryMissing)
        } else if self.is_huge() {
            Err(FrameError::UnexpectedHugePage)
        } else {
            Ok(PhysicalFrame::containing(self.address()))
        }
    }
}
impl PageEntry<Level2> {
    pub const fn set_frame<const N: u64>(self, addr: PhysicalFrame<N>) -> Self
    where
        PageCheck<N>: NotGiantPageSize, // 4Kb or 2Mb
    {
        PageEntry {
            raw: self.raw.set_address(addr.ptr()),
            _level: core::marker::PhantomData,
        }
    }

    pub const fn frame(&self) -> Result<MappedLevel2Page, FrameError> {
        if self.is_present() {
            Err(FrameError::EntryMissing)
        } else if self.is_huge() {
            Ok(MappedLevel2Page::Page2Mb(PhysicalFrame::containing(
                self.address(),
            )))
        } else {
            Ok(MappedLevel2Page::Page4Kb(PhysicalFrame::containing(
                self.address(),
            )))
        }
    }
}
impl PageEntry<Level3> {
    pub const fn set_frame<const N: u64>(self, addr: PhysicalFrame<N>) -> Self
    where
        PageCheck<N>: NotHugePageSize, // 4Kb or 1Gb
    {
        PageEntry {
            raw: self.raw.set_address(addr.ptr()),
            _level: core::marker::PhantomData,
        }
    }

    pub const fn frame(&self) -> Result<MappedLevel3Page, FrameError> {
        if self.is_present() {
            Err(FrameError::EntryMissing)
        } else if self.is_huge() {
            Ok(MappedLevel3Page::Page1Gb(PhysicalFrame::containing(
                self.address(),
            )))
        } else {
            Ok(MappedLevel3Page::Page4Kb(PhysicalFrame::containing(
                self.address(),
            )))
        }
    }
}

impl PageEntry<Level4> {
    pub const fn set_frame(self, addr: PhysicalFrame<Page4Kb>) -> Self {
        PageEntry {
            raw: self.raw.set_address(addr.ptr()),
            _level: core::marker::PhantomData,
        }
    }

    pub const fn frame(&self) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        if self.is_present() {
            Err(FrameError::EntryMissing)
        } else if self.is_huge() {
            Err(FrameError::UnexpectedHugePage)
        } else {
            Ok(PhysicalFrame::containing(self.address()))
        }
    }
}

impl MappedLevel2Page {
    /// Returns `true` if the mapped level2 page is [`Page1Gb`].
    ///
    /// [`Page1Gb`]: MappedLevel2Page::Page1Gb
    pub const fn is_huge(&self) -> bool {
        matches!(self, Self::Page2Mb(..))
    }
}

impl MappedLevel3Page {
    /// Returns `true` if the mapped level3 page is [`Page2Mb`].
    ///
    /// [`Page2Mb`]: MappedLevel3Page::Page2Mb
    pub const fn is_huge(&self) -> bool {
        matches!(self, Self::Page1Gb(..))
    }
}
