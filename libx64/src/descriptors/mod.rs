pub mod call_gate;
pub mod code;
pub mod data;
pub mod interrupt;
pub mod system;
pub mod task;

pub use call_gate::CallGateDescriptor;
pub use code::CodeSegmentDescriptor;
pub use data::DataSegmentDescriptor;
pub use interrupt::InterruptGateDescriptor;
pub use system::{SystemSegmentDescriptor, SystemSegmentType};
pub use task::TaskGateDescriptor;

pub struct GdtNull;

/// SAFETY: they should have the same layout
pub union UserSegment {
    code: CodeSegmentDescriptor,
    data: DataSegmentDescriptor,
}

/// SAFETY: they should have the same layout
pub union GateSegment {
    interrupt: InterruptGateDescriptor,
    task: TaskGateDescriptor,
}

pub enum GdtEntry {
    Null,
    User(UserSegment),
    Gate(GateSegment),
    System(SystemSegmentDescriptor),
}

pub trait AsGdtEntry {
    fn to_gdt_entry(self) -> GdtEntry;
}

macro_rules! as_gdt_impl {
    ($type:ty, $kind:ident, $version:ident, $field:ident) => {
        impl AsGdtEntry for $type {
            #[inline]
            fn to_gdt_entry(self) -> GdtEntry {
                GdtEntry::$kind($version { $field: self })
            }
        }
    };
    ($type:ty, $kind:ident) => {
        impl AsGdtEntry for $type {
            #[inline]
            fn to_gdt_entry(self) -> GdtEntry {
                GdtEntry::$kind(self)
            }
        }
    };
}

impl AsGdtEntry for GdtNull {
    #[inline]
    fn to_gdt_entry(self) -> GdtEntry {
        GdtEntry::Null
    }
}

as_gdt_impl!(CodeSegmentDescriptor, User, UserSegment, code);
as_gdt_impl!(DataSegmentDescriptor, User, UserSegment, data);

as_gdt_impl!(InterruptGateDescriptor, Gate, GateSegment, interrupt);
as_gdt_impl!(TaskGateDescriptor, Gate, GateSegment, task);

as_gdt_impl!(SystemSegmentDescriptor, System);
