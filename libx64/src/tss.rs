use crate::address::VirtualAddr;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct TaskStateSegment {
    _reserved1: u32,

    /// stack pointers for privilege levels 0-2.
    pub rsp: [VirtualAddr; 3],
    _reserved2: u64,
    /// interupt stack table
    pub ist: [VirtualAddr; 7],
    _reserved3: u64,
    _reserved4: u16,
    pub io_map_base: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            _reserved1: 0,
            rsp: [VirtualAddr::null(); 3],
            _reserved2: 0,
            ist: [VirtualAddr::null(); 7],
            _reserved3: 0,
            _reserved4: 0,
            io_map_base: 0,
        }
    }
}
