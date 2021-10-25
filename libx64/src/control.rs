use crate::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{frame::PhysicalFrame, Page4Kb},
};

use bitfield::bitfield;

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
    pub const fn frame(&self) -> PhysicalFrame<Page4Kb> {
        PhysicalFrame::containing(PhysicalAddr::new(self.0 & 0x000F_FFFF_FFFF_F000))
    }
}

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
