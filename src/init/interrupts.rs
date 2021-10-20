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

        // User Interrupts
        idt.user[user::IntIdx::Timer].register(user::timer);
        idt.user[user::IntIdx::Keyboard].register(user::keyboard);

        idt
    };
}

pub extern "x86-interrupt" fn int3(f: InterruptFrame) {
    kprintln!("{:#?}", f)
}

pub extern "x86-interrupt" fn double_fault(f: InterruptFrame, code: u64) -> ! {
    panic!("#DF (code: {}) {:#?}", code, f)
}

pub extern "x86-interrupt" fn page_fault(f: InterruptFrame, code: u64) {
    let code = PageFaultErrorCode::from_bits_truncate(code);
    panic!("#PF (code: {:?}) {:#?}", code, f)
}

#[interrupt_list::interrupt_list(IntIdx)]
pub mod user {
    use super::InterruptFrame;
    use kcore::{klazy, sync::mutex::SpinMutex};
    use pic::chained::ChainedPic;

    klazy! {
        pub ref static PICS: SpinMutex<ChainedPic<0x20, 0x28>> = {
            SpinMutex::new(ChainedPic::<0x20, 0x28>::uninit())
        };
    }

    #[interrupt_list::user_interrupt(32)]
    pub extern "x86-interrupt" fn timer(_f: InterruptFrame) {
        PICS.lock().interupt_fn(IntIdx::Timer).expect("timer");
    }

    #[interrupt_list::user_interrupt(33)]
    pub extern "x86-interrupt" fn keyboard(_f: InterruptFrame) {
        use libx64::port::RPort;

        let kb = RPort::<u8>::new(0x60);
        let _scancode = unsafe { kb.read() };

        PICS.lock().interupt_fn(IntIdx::Keyboard).expect("keyboard");
    }
}
