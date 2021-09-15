use bitfield::{bitfield, BitField};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum SystemSegmentType {
    Ldt = 0x2,
    AvailableTSS = 0x9,
    BusyTSS = 0xB,
    CallGate = 0xC,
    InteruptGate = 0xE,
    TrapGate = 0xF,
}

#[repr(C, packed)]
pub struct SystemSegmentDescriptor {
    limit_low: u16,
    base_low: u16,
    middle_base: u8,
    flags: SsFlags,
    limit_flags: FlagsLimit,
    base_high: u8,
    base_higher: u32,
    reserved: u32,
}

impl SystemSegmentDescriptor {
    pub fn set_type(&mut self, ty: SystemSegmentType) {
        self.flags.set_ss_type(ty as u8);
    }

    pub fn set_present(&mut self) {
        self.flags.set_presence(1);
    }
}

bitfield! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct SsFlags: u8 {
        ss_type: 0..4,
        res2: 4..5,

        /// Descriptor Privilege-Level
        dpl: 5..7,

        /// Presence bit
        presence: 7..8,
    }
}

bitfield! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct FlagsLimit: u8 {
        limit_high: 0..4,

        /// Available To Software (AVL) bit.
        ///
        /// # AMD64 Manual
        ///
        /// Bit 20 of the upper doubleword. This field is available to software, which can write
        /// any value to it. The processor does not set or clear this field.
        avl: 4..5,

        res: 5..7,

        /// Granularity (G) Bit.
        ///
        /// # AMD64 Manual
        ///
        /// Bit 23 of the upper doubleword. The granularity bit specifies how the segment-limit
        /// field is scaled. Clearing the G bit to 0 indicates that the limit field is not scaled.
        /// In this case, the limit equals the number of bytes available in the segment. Setting
        /// the G bit to 1 indicates that the limit field is scaled by 4 Kbytes (4096 bytes). Here,
        /// the limit field equals the number of 4-Kbyte blocks available in the segment.
        granularity: 7..8,
    }
}
