use bitfield::bitfield;

use crate::address::PhysicalAddr;

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageEntry; 512],
}

#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum PageTableLevel {
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Level4 = 4,
}

pub struct PageTableIndex(usize);

impl PageTableIndex {
    pub const fn new_truncate(value: u16) -> Self {
        Self((value as usize) % 512)
    }
}

impl core::ops::Index<PageTableIndex> for PageTable {
    type Output = PageEntry;

    fn index(&self, idx: PageTableIndex) -> &Self::Output {
        &self.entries[idx.0]
    }
}

impl core::ops::IndexMut<PageTableIndex> for PageTable {
    fn index_mut(&mut self, idx: PageTableIndex) -> &mut Self::Output {
        &mut self.entries[idx.0]
    }
}

bitfield! {
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct PageEntry: u64 {
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
        pub page_size: 7..8,

        /// This bit is only present in the lowest level of the page-translation
        /// hierarchy. It indicates the physical page is a global page. The TLB entry for a global page
        /// (G=1) is not invalidated when CR3 is loaded either explicitly by a MOV CRn instruction
        /// or implicitly during a task switch. Use of the G bit requires the page-global enable
        /// bit in CR4 to be set to 1 (CR4.PGE=1).
        pub global_page: 8..9,

        /// These bits are not interpreted by the processor and are available for
        /// use by system software.
        pub avl: 9..10,

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

        address: 12..59,

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

#[derive(Debug)]
pub enum FrameError {
    HugePageNotSupported,
    EntryMissing,
}

impl PageEntry {
    pub const fn address(&self) -> PhysicalAddr {
        PhysicalAddr::new(self.0 & 0x000F_FFFF_FFFF_F000)
    }

    pub const fn set_frame<const N: u64>(self, addr: PhysicalFrame<N>) -> Self
    where
        PageCheck<N>: PageSize,
    {
        self.set_address(addr.ptr().as_u64())
    }

    pub const fn frame<const N: u64>(&self) -> Result<PhysicalFrame<N>, FrameError>
    where
        PageCheck<N>: PageSize,
    {
        if self.get_present() == 0 {
            Err(FrameError::EntryMissing)
        } else if self.get_page_size() != 0 {
            Err(FrameError::HugePageNotSupported)
        } else {
            Ok(PhysicalFrame {
                addr: self.address(),
            })
        }
    }
}

pub trait PageSize {}

#[allow(non_upper_case_globals)]
pub const Page4Kb: u64 = 4096;

#[allow(non_upper_case_globals)]
pub const Page2Mb: u64 = Page4Kb * 512;

#[allow(non_upper_case_globals)]
pub const Page1Gb: u64 = Page2Mb * 512;

pub struct PageCheck<const N: u64>;
impl PageSize for PageCheck<Page4Kb> {}
impl PageSize for PageCheck<Page2Mb> {}
impl PageSize for PageCheck<Page1Gb> {}

#[derive(Debug, Clone, Copy)]
pub struct PhysicalFrame<const N: u64>
where
    PageCheck<N>: PageSize,
{
    addr: PhysicalAddr,
}

impl<const N: u64> PhysicalFrame<N>
where
    PageCheck<N>: PageSize,
{
    pub const fn containing(addr: PhysicalAddr) -> Self {
        Self {
            addr: addr.align_down(N),
        }
    }

    pub const fn ptr(self) -> PhysicalAddr {
        self.addr
    }
}

bitflags::bitflags! {
    /// Describes an page fault error code.
    ///
    /// This structure is defined by the following manual sections:
    ///   * AMD Volume 2: 8.4.2
    ///   * Intel Volume 3A: 4.7
    #[repr(transparent)]
    pub struct PageFaultErrorCode: u64 {
        /// If this flag is set, the page fault was caused by a page-protection violation,
        /// else the page fault was caused by a not-present page.
        const PROTECTION_VIOLATION = 1;

        /// If this flag is set, the memory access that caused the page fault was a write.
        /// Else the access that caused the page fault is a memory read. This bit does not
        /// necessarily indicate the cause of the page fault was a read or write violation.
        const CAUSED_BY_WRITE = 1 << 1;

        /// If this flag is set, an access in user mode (CPL=3) caused the page fault. Else
        /// an access in supervisor mode (CPL=0, 1, or 2) caused the page fault. This bit
        /// does not necessarily indicate the cause of the page fault was a privilege violation.
        const USER_MODE = 1 << 2;

        /// If this flag is set, the page fault is a result of the processor reading a 1 from
        /// a reserved field within a page-translation-table entry.
        const MALFORMED_TABLE = 1 << 3;

        /// If this flag is set, it indicates that the access that caused the page fault was an
        /// instruction fetch.
        const INSTRUCTION_FETCH = 1 << 4;

        /// If this flag is set, it indicates that the page fault was caused by a protection key.
        const PROTECTION_KEY = 1 << 5;

        /// If this flag is set, it indicates that the page fault was caused by a shadow stack
        /// access.
        const SHADOW_STACK = 1 << 6;

        /// If this flag is set, it indicates that the page fault was caused by SGX access-control
        /// requirements (Intel-only).
        const SGX = 1 << 15;

        /// If this flag is set, it indicates that the page fault is a result of the processor
        /// encountering an RMP violation (AMD-only).
        const RMP = 1 << 31;
    }
}
