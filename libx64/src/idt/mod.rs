mod entry;
mod stack;
mod table;

use crate::address::VirtualAddr;

pub use crate::idt::table::InterruptDescriptorTable;
pub use entry::{Entry, Options};
pub use stack::InterruptFrame;

pub fn lidt(idt: &'static InterruptDescriptorTable) {
    #[repr(C, packed)]
    struct IdtPtr {
        limit: u16,
        addr: VirtualAddr,
    }
    let ptr = &IdtPtr {
        limit: (core::mem::size_of::<InterruptDescriptorTable>() - 1) as u16,
        addr: VirtualAddr::new(idt.entries_ptr() as u64).unwrap(),
    };
    // SAFETY: we assure the IDT pointer is well defined
    unsafe {
        asm!("lidt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
    }
}
