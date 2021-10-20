use crate::{
    address::{PhysicalAddr, VirtualAddr},
    paging::PhysicalFrame,
};

use bitfield::{bitfield, BitField};

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
    pub fn frame(&self) -> PhysicalFrame {
        PhysicalFrame::containing(PhysicalAddr::new(self.get_ptr() << 12))
    }
}

pub fn cr3() -> CR3 {
    unsafe {
        let value: u64;
        asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
        CR3::raw(value)
    }
}
