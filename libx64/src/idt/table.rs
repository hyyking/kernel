use crate::address::VirtualAddr;

use crate::idt::entry::Entry;
use crate::idt::stack::InterruptFrame;

#[derive(Debug)]
#[repr(C, align(16))]
pub struct InterruptDescriptorTable {
    pub entries: [Entry; 255],
}

type Handler = extern "x86-interrupt" fn(InterruptFrame);
type CodeHandler = extern "x86-interrupt" fn(InterruptFrame, u64);

impl InterruptDescriptorTable {
    pub const fn new() -> Self {
        Self {
            entries: [Entry::new(); 255],
        }
    }

    pub fn set_handler(&mut self, idx: u8, h: Handler) {
        self.register(idx, VirtualAddr::new(h as u64));
    }

    pub fn set_handler_with_code(&mut self, idx: u8, h: CodeHandler) {
        self.register(idx, VirtualAddr::new(h as u64));
    }

    fn register(&mut self, idx: u8, h: VirtualAddr) {
        let entry = &mut self.entries[usize::from(idx)];

        entry.set_fn_ptr(h);

        // TODO: code segement fetch
        entry.set_cs_sel(Self::get_cs());
        entry.options_mut().set_present(1);
    }

    fn get_cs() -> u16 {
        unsafe {
            let segment: u16;
            asm!("mov {0:x}, cs", out(reg) segment, options(nomem, nostack, preserves_flags));
            segment
        }
    }

    /// Get a reference to the interrupt descriptor table's entries.
    pub fn entries(&self) -> &[Entry] {
        &self.entries[..]
    }
}
