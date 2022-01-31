use crate::boot_info::MemoryRegionKind;
use libx64::address::PhysicalAddr;

/// A physical memory region returned by an `e820` BIOS call.
///
/// See http://wiki.osdev.org/Detecting_Memory_(x86)#Getting_an_E820_Memory_Map for more info.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct E820MemoryRegion {
    pub start_addr: u64,
    pub len: u64,
    pub region_type: u32,
    pub acpi_extended_attributes: u32,
}

impl E820MemoryRegion {
    pub const fn start(&self) -> PhysicalAddr {
        PhysicalAddr::new(self.start_addr)
    }

    pub const fn len(&self) -> u64 {
        self.len
    }

    pub const fn kind(&self) -> MemoryRegionKind {
        match self.region_type {
            1 => MemoryRegionKind::Usable,
            other => MemoryRegionKind::UnknownBios(other),
        }
    }
}
