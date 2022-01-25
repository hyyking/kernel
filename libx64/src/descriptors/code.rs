use bitfield::bitfield;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct CodeSegmentDescriptor {
    limit_low: u16,
    base_low: u16,
    middle_base: u8,
    flags: CsFlags,
    limit_flags: FlagsLimit,
    base_high: u8,
}

impl CodeSegmentDescriptor {
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            middle_base: 0,
            flags: CsFlags::zero(),
            limit_flags: FlagsLimit::zero(),
            base_high: 0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn kernel_x64() -> Self {
        let mut this = Self::empty();
        this.limit_low = u16::MAX;

        this.flags = this
            .flags
            .set_readable(1)
            .set_access(1)
            .set_presence(1)
            .set_res1(1)
            .set_res2(1);

        this.limit_flags = this
            .limit_flags
            .set_granularity(1)
            .set_long(1)
            .set_limit_high(0b1111);

        this
    }
}

bitfield! {
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct CsFlags: u8 {
        access: 0..1,
        readable: 1..2,

        /// Conforming bit
        ///
        /// # AMD64 Manual
        ///
        /// Setting this bit to 1 identifies the code segment
        /// as conforming. When control is transferred to a higher-privilege conforming
        /// code-segment (C=1) from a lower-privilege code segment, the processor CPL does not
        /// change. Transfers to non-conforming code-segments (C = 0) with a higher privilege-level
        /// than the CPL can occur only through gate descriptors. See “Control-Transfer Privilege
        /// Checks” on page 109 for more information on conforming and non-conforming
        /// code-segments.
        conforming: 2..3,
        res1: 3..4,
        res2: 4..5,

        /// Descriptor Privilege-Level
        dpl: 5..7,

        /// Presence bit
        presence: 7..8,
    }
}

bitfield! {
    #[derive(Clone, Copy)]
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

        /// Long mode flag
        long: 5..6,

        /// Code segement default operand size:
        /// - D=0 16bit operand
        /// - D=1 32bit operand
        ///
        /// Must be D=0 if long=1
        operand_size: 6..7,

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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn kernel_segment() {
        let a = unsafe { core::mem::transmute::<_, u64>(CodeSegmentDescriptor::kernel_x64()) };
        assert_eq!(a, 0x00af9b000000ffff);
    }
}
