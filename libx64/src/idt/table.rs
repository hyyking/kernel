use crate::address::VirtualAddr;

use crate::idt::entry::Entry;
use crate::idt::stack::InterruptFrame;

#[derive(Debug)]
#[repr(C, align(16))]
pub struct InterruptDescriptorTable {
    pub entries: [Entry; 255],
}

type Handler = extern "x86-interrupt" fn(InterruptFrame);

impl InterruptDescriptorTable {
    pub const fn new() -> Self {
        Self {
            entries: [Entry::new(); 255],
        }
    }

    pub fn set_handler(&mut self, idx: u8, h: Handler) {
        let entry = &mut self.entries[usize::from(idx)];

        // TODO: refactor this part
        let cs = unsafe {
            let segment: u16;
            asm!("mov {0:x}, cs", out(reg) segment, options(nomem, nostack, preserves_flags));
            segment
        };

        entry.set_fn_ptr(VirtualAddr::new(h as u64).expect("unaligned handler"));
        entry.set_cs_sel(cs);

        entry.options_mut().set_present();
    }

    pub(super) fn entries_ptr(&self) -> *const Entry {
        self.entries.as_ptr()
    }
}
