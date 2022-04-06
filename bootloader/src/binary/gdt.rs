use libx64::{
    address::VirtualAddr,
    descriptors::{CodeSegmentDescriptor, DataSegmentDescriptor, GdtNull},
    gdt::{lgdt, GlobalDescriptorTable},
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError},
        page::{PageMapper, TlbFlush},
        Page4Kb,
    },
    segments::{set_cs, set_ds, set_es, set_ss},
};

#[cold]
pub fn create_and_load<M, A>(kernel_mapper: &mut M, alloc: &mut A) -> Result<(), FrameError>
where
    M: PageMapper<Page4Kb>,
    A: FrameAllocator<Page4Kb>,
{
    let gdt_frame = alloc.alloc()?;
    let phys_addr = gdt_frame.ptr();

    info!("Creating a GDT at {:?}", phys_addr);
    let virt_addr = VirtualAddr::new(phys_addr.as_u64()); // utilize identity mapping

    let ptr = virt_addr.ptr::<GlobalDescriptorTable>().unwrap();

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

    kernel_mapper
        .id_map(gdt_frame, Flags::PRESENT, alloc)
        .map(TlbFlush::ignore)
}
