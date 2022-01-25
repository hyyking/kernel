use core::arch::asm;

use crate::{address::VirtualAddr, descriptors::interrupt::IstIndex};

use bitfield::bitfield;

bitfield! {
    #[derive(Copy, Clone)]
    #[repr(transparent)]
    pub unsafe struct SegmentSelector: u16 {
        pub rpl: 0..2,
        indicator: 2..3,
        pub index: 3..16,
    }
}

bitfield! {
    #[derive(Copy, Clone)]
    #[repr(transparent)]
    pub unsafe struct SegmentSelectorError: u32 {
        pub external: 0..1,
        pub tbl: 1..3,
        pub index: 3..16,
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TaskStateSegment {
    _reserved1: u32,

    /// stack pointers for privilege levels 0-2.
    pub rsp: [VirtualAddr; 3],

    _reserved2: u64,
    /// interupt stack table
    pub ist: IstEntries,
    _reserved3: u64,
    _reserved4: u16,
    pub io_map_base: u16,
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct IstEntries {
    entries: [VirtualAddr; 7],
}

impl<T> core::ops::Index<T> for IstEntries
where
    T: Into<IstIndex>,
{
    type Output = VirtualAddr;

    fn index(&self, index: T) -> &Self::Output {
        &self.entries[usize::from(index.into())]
    }
}

impl<T> core::ops::IndexMut<T> for IstEntries
where
    T: Into<IstIndex>,
{
    fn index_mut(&mut self, index: T) -> &mut Self::Output {
        &mut self.entries[usize::from(index.into())]
    }
}

#[inline]
pub fn ltr(sel: SegmentSelector) {
    unsafe {
        asm!("ltr {0:x}", in(reg) sel.0, options(nomem, nostack, preserves_flags));
    }
}

impl TaskStateSegment {
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            _reserved1: 0,
            rsp: [VirtualAddr::null(); 3],
            _reserved2: 0,
            ist: IstEntries {
                entries: [VirtualAddr::null(); 7],
            },
            _reserved3: 0,
            _reserved4: 0,
            io_map_base: 0,
        }
    }
}

/// | Segment Register | Description                                       |
/// |------------------|---------------------------------------------------|
/// | ES               | References optional data-segment descriptor entry |
/// | CS               | References code-segment descriptor entry          |
/// | SS               | References stack segment descriptor entry         |
/// | DS               | References default data-segment descriptor entry  |
/// | FS               | References optional data-segment descriptor entry |
/// | GS               | References optional data-segment descriptor entry |

#[inline]
#[must_use]
pub fn es() -> u16 {
    unsafe {
        let segment: u16;
        asm!("mov {0:x}, es", out(reg) segment, options(nomem, nostack, preserves_flags));
        segment
    }
}

#[inline]
pub fn set_es(es: SegmentSelector) {
    unsafe {
        asm!("mov es, {0:x}", in(reg) es.0, options(nostack, preserves_flags));
    }
}

#[inline]
#[must_use]
pub fn cs() -> u16 {
    unsafe {
        let segment: u16;
        asm!("mov {0:x}, cs", out(reg) segment, options(nomem, nostack, preserves_flags));
        segment
    }
}

pub fn set_cs(ss: SegmentSelector) {
    unsafe {
        asm!(
            "push {sel}",
            "lea {tmp}, [1f + rip]",
            "push {tmp}",
            "retfq",
            "1:",
            sel = in(reg) u64::from(ss.0),
            tmp = lateout(reg) _,
            options(preserves_flags)
        );
    }
}

#[inline]
#[must_use]
pub fn ss() -> u16 {
    unsafe {
        let segment: u16;
        asm!("mov {0:x}, ss", out(reg) segment, options(nomem, nostack, preserves_flags));
        segment
    }
}

#[inline]
pub fn set_ss(ss: SegmentSelector) {
    unsafe {
        asm!("mov ss, {0:x}", in(reg) ss.0, options(nostack, preserves_flags));
    }
}

#[inline]
#[must_use]
pub fn ds() -> u16 {
    unsafe {
        let segment: u16;
        asm!("mov {0:x}, ds", out(reg) segment, options(nomem, nostack, preserves_flags));
        segment
    }
}

#[inline]
pub fn set_ds(ds: SegmentSelector) {
    unsafe {
        asm!("mov ds, {0:x}", in(reg) ds.0, options(nostack, preserves_flags));
    }
}

#[inline]
#[must_use]
pub fn fs() -> u16 {
    unsafe {
        let segment: u16;
        asm!("mov {0:x}, fs", out(reg) segment, options(nomem, nostack, preserves_flags));
        segment
    }
}

#[inline]
#[must_use]
pub fn gs() -> u16 {
    unsafe {
        let segment: u16;
        asm!("mov {0:x}, gs", out(reg) segment, options(nomem, nostack, preserves_flags));
        segment
    }
}
