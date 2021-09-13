use crate::address::VirtualAddr;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct InterruptFrame {
    instruction_ptr: VirtualAddr,
    code_segment: u64,
    rflags: u64,
    stack_pointer: VirtualAddr,
    segment_selector: u64,
}

impl InterruptFrame {
    /// Get a reference to the interrupt frame's instruction ptr.
    pub fn instruction_ptr(&self) -> VirtualAddr {
        self.instruction_ptr
    }

    /// Get a reference to the interrupt frame's code segment.
    pub fn code_segment(&self) -> u64 {
        self.code_segment
    }

    /// Get a reference to the interrupt frame's rflags.
    pub fn rflags(&self) -> u64 {
        self.rflags
    }

    /// Get a reference to the interrupt frame's stack pointer.
    pub fn stack_pointer(&self) -> VirtualAddr {
        self.stack_pointer
    }

    /// Get a reference to the interrupt frame's segment selector.
    pub fn segment_selector(&self) -> u64 {
        self.segment_selector
    }
}
