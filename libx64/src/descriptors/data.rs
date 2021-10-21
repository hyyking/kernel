use bitfield::bitfield;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DataSegmentDescriptor {
    limit_low: u16,
    base_low: u16,
    middle_base: u8,
    flags: DsFlags,
    limit_flags: FlagsLimit,
    base_high: u8,
}

bitfield! {
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct DsFlags: u8 {
        access: 0..1,
        writable: 1..2,

        /// Expand-Down (E) bit
        ///
        /// # AMD64 Manual
        ///
        /// Setting this bit to 1 identifies the data segment as expand-down. In expand-down
        /// segments, the segment limit defines the lower segment boundary while the base is the
        /// upper boundary. Valid segment offsets in expand-down segments lie in the byte range
        /// limit+1 to FFFFh or FFFF_FFFFh, depending on the value of the data segment default
        /// operand size (D/B) bit.
        expand_down: 2..3,

        res1: 3..4, // must be set to 0 in long mode
        res2: 4..5, // must be set to 1 in long mode

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

        res1: 5..6, // 0 in long mode

        /// Data-Segment Default Operand Size (D/B) Bit.
        ///
        /// # AMD64 Manual
        ///
        /// For expand-down data segments (E=1), setting D=1 sets the upper bound of the segment at
        /// 0_FFFF_FFFFh. Clearing D=0 sets the upper bound of the segment at 0_FFFFh. In the case
        /// where a data segment is referenced by the stack selector (SS), the D bit is referred to
        /// as the B bit. For stack segments, the B bit sets the default stack size. Setting B=1
        /// establishes a 32-bit stack referenced by the 32-bit ESP register. Clearing B=0
        /// establishes a 16-bit stack referenced by the 16-bit SP register.
        dl: 6..7,

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
