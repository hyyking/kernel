use crate::{
    address::{PhysicalAddr, VirtualAddr},
    paging::{PageCheck, PageSize, PhysicalFrame},
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
    pub const fn frame<const N: u64>(&self) -> PhysicalFrame<N>
    where
        PageCheck<N>: PageSize,
    {
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
