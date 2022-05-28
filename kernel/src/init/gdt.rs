use libx64::{
    address::VirtualAddr,
    descriptors::{CodeSegmentDescriptor, GdtNull, SystemSegmentDescriptor},
    gdt::GlobalDescriptorTable,
    segments::TaskStateSegment,
};

use kcore::tables::{gdt::Selectors, idt::IstEntry};

klazy! {
    pub ref static GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        gdt.add_entry(GdtNull);
        let task_state = gdt.add_entry(SystemSegmentDescriptor::from(&*TSS));
        let code_segment = gdt.add_entry(CodeSegmentDescriptor::kernel_x64());

        (gdt, Selectors {
                code_segment,
                task_state
        })
    };
}

klazy! {
    pub ref static TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::zero();

        tss.ist[IstEntry::DoubleFault] = {
            const STACK_SIZE: usize = 4096 * 8; // 32Kb stack

            #[repr(align(16))]
            pub struct Stack([u8; STACK_SIZE]);
            static mut STACK: Stack = Stack([0; STACK_SIZE]);

            // SAFETY: Stack grows down
            VirtualAddr::from_ptr(unsafe { STACK.0.as_ptr().add(STACK_SIZE) })
        };

        tss
    };
}
