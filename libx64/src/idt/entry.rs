use crate::address::VirtualAddr;

use bitfield::{bitfield, BitField};

bitfield! {
    // SAFETY: no bits are overlaping
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct Options: u16 {
        /// Interupt stack table index
        pub ist: 0..3,

        /// reserved and always 0
        reserved1: 3..8,

        /// gate types:
        /// 0b0101 	0x5 	5 	80386 32 bit task gate
        /// 0b0110 	0x6 	6 	80286 16-bit interrupt gate
        /// 0b0111 	0x7 	7 	80286 16-bit trap gate
        /// 0b1110 	0xE 	14 	80386 32-bit interrupt gate
        /// 0b1111 	0xF 	15 	80386 32-bit trap gate
        pub gate_type: 8..12,

        /// reserved and always 0
        reserved2: 12..13,

        /// Descriptor privilege level
        pub dpl: 13..15,

        /// Presence flag, set to 0 for unused interrupts.
        pub present: 15..16,
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Entry {
    pointer_low: u16,
    gdt_selector: u16,
    options: Options,
    pointer_middle: u16,
    pointer_high: u32,
    _reserved: u32,
}

impl Entry {
    pub const fn new() -> Self {
        Self {
            pointer_low: 0,
            gdt_selector: 0,
            options: Options::empty(),
            pointer_middle: 0,
            pointer_high: 0,
            _reserved: 0,
        }
    }

    pub fn set_fn_ptr(&mut self, addr: VirtualAddr) {
        let addr = addr.as_u64();
        self.pointer_low = addr as u16;
        self.pointer_middle = (addr >> 16) as u16;
        self.pointer_high = (addr >> 32) as u32;
    }

    pub fn set_cs_sel(&mut self, sel: u16) {
        self.gdt_selector = sel;
    }

    /// Get a mutable reference to the entry's options.
    pub fn options_mut(&mut self) -> &mut Options {
        &mut self.options
    }
}

impl Options {
    /// Default setup:
    /// - ist: 0
    /// - gate_type: 0b1110 (Interupt gate)
    /// - dpl: 0
    /// - presence: 0 (not present)
    pub const fn empty() -> Self {
        Self(0b0000_1110_0000_0000)
    }
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let handler = self.pointer_low as u64
            | (self.pointer_middle as u64) << 16
            | (self.pointer_high as u64) << 32;
        f.debug_struct("IdtEntry")
            .field("handler", &format_args!("{:#02x}", handler))
            .field("gdt_sel", &self.gdt_selector)
            .field("options", &format_args!("{:#0b}", self.options.0))
            .finish()
    }
}
