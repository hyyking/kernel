use crate::address::VirtualAddr;

/// Default setup:
///
/// 0000 0110 0000 0000
/// ^    ^  ^
/// |^^  |  Fault/Trap gate
/// ||/  |                      
/// ||   Gate size 1 = 32b / 0 = 16b                    
/// ||                          
/// |Descriptor Privilege Level
/// |
/// Presence flag
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Options {
    bits: u16,
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
    pub const fn empty() -> Self {
        Self {
            bits: 0b0000_1110_0000_0000,
        }
    }

    pub const fn dpl(&self) -> u16 {
        (self.bits & (2 << 13)) >> 13
    }

    pub const fn present(&self) -> bool {
        (self.bits & (1 << 15)) != 0
    }

    pub fn set_present(&mut self) {
        self.bits |= 1 << 15
    }

    pub const fn trap_gate(&self) -> bool {
        // 0
        (self.bits & (1 << 8)) != 0
    }

    pub const fn gate_size(&self) -> u8 {
        match (self.bits & (1 << 11)) > 0 {
            true => 32,
            false => 16,
        }
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
            .field("options", &format_args!("{:#0b}", self.options.bits))
            .finish()
    }
}
