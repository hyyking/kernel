mod entry;
mod table;

pub use table::GlobalDescriptorTable;

use crate::address::VirtualAddr;

pub fn lgdt(gdt: &'static GlobalDescriptorTable) {
    #[repr(C, packed)]
    struct GdtPtr {
        limit: u16,
        addr: VirtualAddr,
    }
    let ptr = &GdtPtr {
        limit: (gdt.entries().len() - 1) as u16,
        addr: VirtualAddr::from_ptr(gdt.entries().as_ptr()),
    };
    // SAFETY: we assure the IDT pointer is well defined
    unsafe {
        asm!("lgdt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
    }
}
