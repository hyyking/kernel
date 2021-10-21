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
    qprintln!("[OK] GDT Initialized");

    set_cs(segments.code_segment);
    ltr(segments.task_state);
    qprintln!("[OK] Segments switched");

    lidt(&interrupts::IDT.lidt_ptr());
    qprintln!("[OK] IDT Initialized");

    interrupts::user::PICS
        .lock()
        .init()
        .expect("failed to initialize PIC");

    qprintln!("[OK] PIC Initialized");

    qprintln!("[OK] initialization successful");
}
