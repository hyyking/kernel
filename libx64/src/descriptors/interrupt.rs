use crate::address::VirtualAddr;
use crate::descriptors::system::SystemSegmentType;

use bitfield::bitfield;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct InterruptGateDescriptor {
    target_low: u16,
    selector: u16,
    flags: IgFlags,
    target_middle: u16,
    target_high: u32,
    reserved: u32,
}

#[repr(u16)]
pub enum IstIndex {
    Idx1 = 1,
    Idx2 = 2,
    Idx3 = 3,
    Idx4 = 4,
    Idx5 = 5,
    Idx6 = 6,
    Idx7 = 7,
}

impl From<IstIndex> for usize {
    fn from(val: IstIndex) -> Self {
        usize::from(val as u16)
    }
}

bitfield! {
    // SAFETY: no bits are overlaping
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct IgFlags: u16 {
        /// Interupt stack table index (values: 0-7)
        ist: 0..3,

        /// reserved and always 0
        reserved1: 3..8,

        pub gate_type: 8..12,

        /// reserved and always 0
        reserved2: 12..13,

        /// Descriptor privilege level
        pub dpl: 13..15,

        /// Presence flag, set to 0 for unused interrupts.
        pub present: 15..16,
    }
}

impl IgFlags {
    #[inline]
    pub fn set_stack_idx(&mut self, val: impl Into<IstIndex>) -> &mut Self {
        *self = self.set_ist(val.into() as u16);
        self
    }
}

impl InterruptGateDescriptor {
    pub const fn new() -> Self {
        Self {
            target_low: 0,
            selector: 0,
            flags: IgFlags((SystemSegmentType::InterruptGate as u16) << 8),
            target_middle: 0,
            target_high: 0,
            reserved: 0,
        }
    }

    pub fn get_target(&self) -> VirtualAddr {
        VirtualAddr::new(
            self.target_low as u64
                | (self.target_middle as u64) << 16
                | (self.target_high as u64) << 32,
        )
    }

    pub fn set_target(&mut self, addr: VirtualAddr) {
        let addr = addr.as_u64();
        self.target_low = addr as u16;
        self.target_middle = (addr >> 16) as u16;
        self.target_high = (addr >> 32) as u32;
    }

    pub fn set_selector(&mut self, sel: u16) {
        self.selector = sel;
    }

    /// Get a mutable reference to the entry's options.
    pub fn flags_mut(&mut self) -> &mut IgFlags {
        &mut self.flags
    }
}

impl core::fmt::Debug for InterruptGateDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InteruptGateDescriptor")
            .field("handler", &self.get_target())
            .field("selector", &{ self.selector })
            .field("options", &self.flags)
            .finish()
    }
}
