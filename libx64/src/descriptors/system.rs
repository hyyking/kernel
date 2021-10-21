use bitfield::bitfield;

use crate::address::VirtualAddr;
use crate::segments::TaskStateSegment;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum SystemSegmentType {
    Ldt = 0x2,
    AvailableTSS = 0x9,
    BusyTSS = 0xB,
    CallGate = 0xC,
    InterruptGate = 0xE,
    TrapGate = 0xF,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct SystemSegmentDescriptor {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    flags: SsFlags,
    limit_flags: FlagsLimit,
    base_high: u8,
    base_higher: u32,
    reserved: u32,
}

impl SystemSegmentDescriptor {
    pub fn zero() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            base_middle: 0,
            flags: SsFlags::zero(),
            limit_flags: FlagsLimit::zero(),
            base_high: 0,
            base_higher: 0,
            reserved: 0,
        }
    }

    pub fn get_base(&self) -> VirtualAddr {
        VirtualAddr::new(
            self.base_low as u64
                | (self.base_middle as u64) << 16
                | (self.base_high as u64) << 24
                | (self.base_higher as u64) << 32,
        )
    }

    pub fn set_base(&mut self, addr: VirtualAddr) {
        let addr = addr.as_u64();
        self.base_low = addr as u16;
        self.base_middle = (addr >> 16) as u8;
        self.base_high = (addr >> 24) as u8;
        self.base_higher = (addr >> 32) as u32;
    }

    pub const fn get_limit(&self) -> u32 {
        self.limit_low as u32 | ((self.limit_flags.get_limit_high() as u32) << 16)
    }

    pub fn set_limit(&mut self, limit: u32) {
        self.limit_low = limit as u16;
        self.limit_flags = self.limit_flags.set_limit_high((limit >> 16) as u8);
    }

    pub fn set_type(&mut self, ty: SystemSegmentType) {
        self.flags = self.flags.set_ss_type(ty as u8);
    }

    pub fn set_present(&mut self) {
        self.flags = self.flags.set_presence(1);
    }
}

bitfield! {
    #[derive(Clone, Copy)]
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

impl From<&TaskStateSegment> for SystemSegmentDescriptor {
    fn from(tss: &TaskStateSegment) -> Self {
        let mut ss = SystemSegmentDescriptor::zero();

        ss.set_base(VirtualAddr::from_ptr(tss));
        ss.set_limit(core::mem::size_of_val(tss).saturating_sub(1) as u32);
        ss.set_type(SystemSegmentType::AvailableTSS);
        ss.set_present();

        ss
    }
}

impl core::fmt::Debug for SystemSegmentDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let base = self.get_base();
        let limit = self.get_limit();

        f.debug_struct("SystemSegmentDescriptor")
            .field("base", &base)
            .field("limit", &{ limit })
            .field("options", &self.flags)
            .field("avl", &(self.limit_flags.get_avl() != 0))
            .field("res", &(self.limit_flags.get_res() != 0))
            .field("granularity", &(self.limit_flags.get_granularity() != 0))
            .finish()
    }
}
