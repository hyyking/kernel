use libx64::{
    idt::{InterruptDescriptorTable, InterruptFrame},
    paging::PageFaultErrorCode,
};

use kcore::tables::idt::IstEntry;

klazy! {
    pub ref static IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // Predefined Interrupts
        idt.breakpoint.register(int3);
        idt.double_fault.register(double_fault).set_stack_idx(IstEntry::DoubleFault);
        idt.page_fault.register(page_fault);
        idt.general_protection.register(general_protection);
        idt.invalid_tss.register(invalid_tss);

        // User Interrupts
        idt.user[user::IntIdx::Timer].register(user::timer);
        idt.user[user::IntIdx::Keyboard].register(user::keyboard);

        idt
    };
}

pub extern "x86-interrupt" fn int3(f: InterruptFrame) {
    // kprintln!("{:#?}", f);
}

pub extern "x86-interrupt" fn double_fault(f: InterruptFrame, code: u64) -> ! {
    panic!("#DF (code: {}) {:#?}", code, f);
}

pub extern "x86-interrupt" fn invalid_tss(f: InterruptFrame, code: u64) {
    panic!("#TS (code: {}) {:#?}", code, f);
}

pub extern "x86-interrupt" fn general_protection(f: InterruptFrame, code: u64) {
    let code = unsafe { libx64::segments::SegmentSelectorError::raw(code as u32) };
    panic!("#GP\nCode: {:#?}\nFrame: {:#?}", code, f);
}

pub extern "x86-interrupt" fn page_fault(f: InterruptFrame, code: u64) {
    let code = PageFaultErrorCode::from_bits_truncate(code);
    panic!("#PF (code: {:?}) {:#?}", code, f);
}

#[interrupt_list::interrupt_list(IntIdx)]
pub mod user {
    use super::{super::KEYBOARD, InterruptFrame};
    use kcore::{klazy, sync::SpinMutex};
    use pic::chained::Chained;

    klazy! {
        pub ref static PICS: SpinMutex<Chained<0x20, 0x28>> = {
            SpinMutex::new(Chained::<0x20, 0x28>::uninit())
        };
    }

    #[interrupt_list::user_interrupt(32)]
    pub extern "x86-interrupt" fn timer(f: InterruptFrame) {
        drop(f);
        PICS.lock().interupt_fn(IntIdx::Timer).expect("timer");
    }

    #[interrupt_list::user_interrupt(33)]
    pub extern "x86-interrupt" fn keyboard(_f: InterruptFrame) {
        use libx64::port::RPort;

        static KB: RPort<u8> = RPort::new(0x60);

        unsafe { dbg!(KB.read()) };

        PICS.lock().interupt_fn(IntIdx::Keyboard).expect("keyboard");
    }
}
