use core::arch::asm;

use crate::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{frame::{PhysicalFrame, FrameTranslator}, Page4Kb, PinTableMut, table::{Level4, PageTable}},
};

use bitfield::bitfield;

#[inline]
#[must_use]
pub fn cr2() -> VirtualAddr {
    unsafe {
        let value: u64;
        asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
        VirtualAddr::new(value)
    }
}

bitfield! {
    pub unsafe struct CR3: u64 {
        pub pwd: 3..4,
        pub pcd: 4..5,
        ptr: 12..52,
    }
}

impl CR3 {
    #[inline]
    #[must_use]
    pub const fn frame(&self) -> PhysicalFrame<Page4Kb> {
        PhysicalFrame::containing(PhysicalAddr::new(self.0 & 0x000F_FFFF_FFFF_F000))
    }

    pub fn table<'a>(self, translator: &dyn FrameTranslator<(), Page4Kb>) -> PinTableMut<'a, Level4>  {
        PageTable::new(self, translator)
    }
}

#[inline]
#[must_use]
pub fn cr3() -> CR3 {
    unsafe {
        let value: u64;
        asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
        CR3::raw(value)
    }
}

pub fn set_cr3(cr3: CR3) {
    unsafe {
        asm!("mov cr3, {}", in(reg) cr3.as_u64(), options(nomem, nostack, preserves_flags));
    }
}

bitflags::bitflags! {
    /// Extended Feature Enable Register
    pub struct Efer: u64 {
        /// Sytem call extensions
        const SCE = 1;
        /// Long mode enable
        const LME = 1 << 9;
        /// Long mode active
        const LMA = 1 << 10;
        /// No-execute active
        const NXE = 1 << 11;
        /// Secure virtual machine enable
        const SVME = 1 << 12;
        /// Long Mode Segment Limit Enable
        const LMSLE = 1 << 13;
        /// Fast FXSAVE/FXRSTOR
        const FFXSR = 1 << 14;
        /// Translation Cache Extension
        const TCE = 1 << 15;
        /// Enable MCOMMIT instruction
        const MCOMMIT = 1 << 17;
        /// Interruptible WBINVD/WBNOINVD
        const INTWB = 1 << 18;
    }
}

const EFER_MSR: u32 = 0xC000_0080;

#[inline]
#[must_use]
pub fn efer() -> Efer {
    let (high, low): (u32, u32);
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") EFER_MSR,
            out("eax") low, out("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
    Efer::from_bits_truncate(((high as u64) << 32) | (low as u64))
}

#[inline]
pub fn set_efer(efer: Efer) {
    let value = efer.bits();
    let low = value as u32;
    let high = (value >> 32) as u32;

    unsafe {
        asm!(
            "wrmsr",
            in("ecx") EFER_MSR,
            in("eax") low, in("edx") high,
            options(nostack, preserves_flags),
        );
    }
}

bitflags::bitflags! {
    pub struct CR0: u64 {
        /// Protection Enable
        const PE = 1;
        /// Monitor Coprocessor
        const MP = 1 << 1;
        /// Emulation
        const EM = 1 << 2;
        /// Task Switched
        const TS = 1 << 3;
        /// Extension Type
        const ET = 1 << 4;
        /// Numeric Error
        const NE = 1 << 5;
        /// Write Protect
        const WP = 1 << 16;
        /// Alignement Mask
        const AM = 1 << 18;
        /// Not Writethrough
        const NW = 1 << 29;
        /// Cache disable
        const CD = 1 << 30;
        /// Paging
        const PG = 1 << 31;
    }
}

pub fn cr0() -> CR0 {
    let value: u64;
    unsafe {
        asm!("mov {}, cr0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    CR0::from_bits_truncate(value)
}

pub fn set_cr0(cr0: CR0) {
    unsafe {
        asm!("mov cr0, {}", in(reg) cr0.bits(), options(nostack, preserves_flags));
    }
}
