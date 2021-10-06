use bitfield::{bitfield, BitField};

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct TaskGateDescriptor {
    _reserved1: u16,
    pub tss_selector: u16,
    _reserved2: u8,
    pub flags: CgFlags,
    _reserved3: u16,
}

bitfield! {
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct CgFlags: u8 {

        // These  bits  are  encoded  by software as 00101b to indicate a task-gate descriptor type
        ss_type: 0..4,
        system: 4..5,

        /// Descriptor Privilege-Level
        dpl: 5..7,

        /// Presence bit
        presence: 7..8,
    }
}
