use libx64::address::VirtualAddr;
use libx64::descriptors::{CodeSegmentDescriptor, DataSegmentDescriptor, GdtNull};
use libx64::gdt::{lgdt, GlobalDescriptorTable};
use libx64::paging::frame::PhysicalFrame;
use libx64::paging::Page4Kb;
use libx64::segments::{set_cs, set_ds, set_es, set_ss};

pub fn create_and_load(frame: PhysicalFrame<Page4Kb>) {
    let phys_addr = frame.ptr();

    log::info!("Creating GDT at {:?}", phys_addr);
    let virt_addr = VirtualAddr::new(phys_addr.as_u64()); // utilize identity mapping

    let ptr: core::ptr::NonNull<GlobalDescriptorTable> = virt_addr.ptr().unwrap();

    let mut gdt = GlobalDescriptorTable::new();

    gdt.add_entry(GdtNull);
    let code_selector = gdt.add_entry(CodeSegmentDescriptor::kernel_x64());
    let data_selector = gdt.add_entry(DataSegmentDescriptor::kernel_x64());

    let gdt = unsafe {
        ptr.as_ptr().write(gdt);
        ptr.as_ref()
    };

    lgdt(&gdt.lgdt_ptr());

    set_cs(code_selector);
    set_ds(data_selector);
    set_es(data_selector);
    set_ss(data_selector);
}
