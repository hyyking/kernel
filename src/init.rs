use libx64::address::VirtualAddr;
use libx64::idt::{lidt, InterruptDescriptorTable as Idt, InterruptFrame};
use libx64::tss::TaskStateSegment;

klazy! {
    ref static IDT: Idt = {
        let mut idt = Idt::new();
        idt.set_handler(0x03, int3);
        idt.set_handler_with_code(0x08, double_fault);
        idt
    };
}

#[repr(u8)]
enum IstEntry {
    DoubleFault = 0,
}
impl From<IstEntry> for usize {
    fn from(e: IstEntry) -> Self {
        usize::from(e as u8)
    }
}

klazy! {
    ref static TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.ist[usize::from(IstEntry::DoubleFault)] = unsafe {
            const STACK_SIZE: usize = 4096 * 2; // 2MB stack
            static STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            VirtualAddr::from_ptr((&STACK as *const u8).add(STACK_SIZE))
        };
        tss
    };
}

pub fn kinit() {
    lidt(&IDT);
    qprintln!("[OK] IDT Initialized");
    qprintln!("[OK] TSS Initialized");
}

pub extern "x86-interrupt" fn int3(f: InterruptFrame) {
    kprintln!("{:#?}", f)
}

pub extern "x86-interrupt" fn double_fault(f: InterruptFrame, code: u64) {
    panic!("{:#?}\n{}", f, code)
}
