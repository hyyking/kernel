use libx64::{
    address::VirtualAddr,
    descriptors::{CodeSegmentDescriptor, GdtNull, SystemSegmentDescriptor},
    gdt::{lgdt, GlobalDescriptorTable},
    idt::{lidt, InterruptDescriptorTable, InterruptFrame},
    segments::{ltr, set_cs, TaskStateSegment},
};

use pic::chained::ChainedPic;

use kcore::{
    sync::mutex::SpinMutex,
    tables::{gdt::Selectors, idt::IstEntry},
};

#[inline(never)]
pub fn kinit() {
    let (gdt, segments) = &*GDT;

    lgdt(&gdt.lgdt_ptr());
    qprintln!("[OK] GDT Initialized");

    set_cs(segments.code_segment);
    ltr(segments.task_state);
    qprintln!("[OK] Segments switched");

    lidt(&IDT.lidt_ptr());
    qprintln!("[OK] IDT Initialized");

    PICS.lock().init().expect("failed to initialize PIC");
    qprintln!("[OK] PIC Initialized");

    qprintln!("[OK] initialization successful");
}

klazy! {
    ref static PICS: SpinMutex<ChainedPic<0x20, 0x28>> = {
        SpinMutex::new(ChainedPic::<0x20, 0x28>::uninit())
    };
}

klazy! {
    ref static IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.register(int3);
        idt.double_fault.register(double_fault).set_stack_idx(IstEntry::DoubleFault);

        idt.user[0].register(timer);

        idt
    };
}

klazy! {
    ref static GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        gdt.add_entry(GdtNull);
        let code_segment = gdt.add_entry(CodeSegmentDescriptor::kernel_x64());
        let task_state = gdt.add_entry(SystemSegmentDescriptor::from(&*TSS));

        (gdt, Selectors {
            code_segment,
            task_state
        })
    };
}

klazy! {
    ref static TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::zero();

        tss.ist[IstEntry::DoubleFault] = {
            const STACK_SIZE: usize = 4096 * 8; // 4Mb stack
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            // SAFETY: Stack grows down
            VirtualAddr::from_ptr(unsafe { STACK.as_ptr().add(STACK_SIZE) })
        };

        tss
    };
}

// __________ INTERRUPTS __________
pub extern "x86-interrupt" fn int3(f: InterruptFrame) {
    kprintln!("{:#?}", f)
}

pub extern "x86-interrupt" fn timer(_f: InterruptFrame) {
    qprintln!(".");
    PICS.lock().interupt_fn(32, || {}).unwrap();
}

pub extern "x86-interrupt" fn double_fault(f: InterruptFrame, code: u64) -> ! {
    // kprintln!("{:#?}\ncode:{}", f, code)
    panic!("DOUBLE FAULT:\n> {:#?}\n> error_code: {}", f, code)
}
