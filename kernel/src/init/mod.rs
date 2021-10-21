mod gdt;
mod interrupts;

use libx64::{
    gdt::lgdt,
    idt::lidt,
    segments::{ltr, set_cs},
};

#[inline(never)]
pub fn kinit() {
    let (gdt, segments) = &*gdt::GDT;

    lgdt(&gdt.lgdt_ptr());
    trace!("GDT Initialized");

    set_cs(segments.code_segment);
    ltr(segments.task_state);
    trace!("Segments switched");

    lidt(&interrupts::IDT.lidt_ptr());
    trace!("IDT Initialized");

    interrupts::user::PICS
        .lock()
        .init()
        .expect("failed to initialize PIC");

    trace!("PIC Initialized");

    info!("initialization successful");
}