use crate::descriptors::system::SystemSegmentType;

use bitfield::{bitfield, BitField};

#[repr(C, packed)]
pub struct CallGateDescriptor {
    offset_low: u16,
    selector: u16,
    _reserved1: u8,
    flags: CgFlags,
    offset_middle: u16,
    offset_high: u32,
    _reserved2: u32,
}

impl CallGateDescriptor {
    pub const fn new() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            _reserved1: 0,
            flags: CgFlags(0b0000_0000 | SystemSegmentType::CallGate as u8),
            offset_middle: 0,
            offset_high: 0,
            _reserved2: 0,
        }
    }
}

bitfield! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct CgFlags: u8 {
        ss_type: 0..4,
        res2: 4..5,

        /// Descriptor Privilege-Level
        dpl: 5..7,

        /// Presence bit
        presence: 7..8,
    }
}
