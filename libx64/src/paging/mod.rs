use core::pin::Pin;

use crate::{
    address::VirtualAddr,
    units::bits::{Gb, Kb, Mb},
};

pub mod entry;
pub mod frame;
pub mod page;
pub mod table;

pub type PinTableMut<'a, L> = Pin<&'a mut table::PageTable<L>>;
pub type PinEntryMut<'a, L> = Pin<&'a mut entry::PageEntry<L>>;

#[allow(non_upper_case_globals)]
pub const Page4Kb: u64 = 4 * Kb;

#[allow(non_upper_case_globals)]
pub const Page2Mb: u64 = 2 * Mb;

#[allow(non_upper_case_globals)]
pub const Page1Gb: u64 = 1 * Gb;

pub trait PageSize {}
pub trait NotHugePageSize: PageSize {}
pub trait NotGiantPageSize: PageSize {}

pub struct PageCheck<const N: u64>;

impl NotHugePageSize for PageCheck<Page4Kb> {}
impl NotHugePageSize for PageCheck<Page1Gb> {}

impl NotGiantPageSize for PageCheck<Page4Kb> {}
impl NotGiantPageSize for PageCheck<Page2Mb> {}

impl PageSize for PageCheck<Page4Kb> {}
impl PageSize for PageCheck<Page2Mb> {}
impl PageSize for PageCheck<Page1Gb> {}

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
        const PROTECTION_VIOLATION = 1 << 0;

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

#[inline]
pub fn invlpg(addr: VirtualAddr) {
    unsafe {
        asm!("invlpg [{}]", in(reg) addr.as_u64(), options(nostack, preserves_flags));
    }
}

/// Invalidate the TLB completely by reloading the CR3 register.
#[inline]
pub fn invalidate_tlb() {
    let cr3 = crate::control::cr3();
    crate::control::set_cr3(cr3)
}
