use core::marker::PhantomData;

use crate::{
    address::VirtualAddr,
    descriptors::{interrupt::IgFlags, InterruptGateDescriptor},
    segments::cs,
};

type Handler = extern "x86-interrupt" fn(InterruptFrame);
type CodeHandler = extern "x86-interrupt" fn(InterruptFrame, u64);
type DivergingCodeHandler = extern "x86-interrupt" fn(InterruptFrame, u64) -> !;

macro_rules! impl_register_handler {
    ($($h:ty)*) => {
        $(
            impl Entry<$h> {
                pub fn register(&mut self, h: $h) -> &mut IgFlags {
                    self.set_target(VirtualAddr::new(h as u64));
                    self.set_selector(cs());
                    self.flags_mut().set_present(u16::from(true));
                    self.flags_mut()
                }
            }
        )*

    }
}

impl_register_handler!(Handler CodeHandler DivergingCodeHandler);

#[derive(Debug, Clone)]
#[repr(C)]
pub struct InterruptFrame {
    pub instruction_ptr: VirtualAddr,
    pub code_segment: u64,
    pub rflags: u64,
    pub stack_pointer: VirtualAddr,
    pub segment_selector: u64,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Entry<H> {
    igd: InterruptGateDescriptor,
    _m: core::marker::PhantomData<H>,
}

impl<H> Entry<H> {
    pub const fn new() -> Self {
        Self {
            igd: InterruptGateDescriptor::new(),
            _m: core::marker::PhantomData,
        }
    }
}

#[derive(Debug)]
#[repr(C, align(16))]
pub struct InterruptDescriptorTable {
    pub divide_by_zero: Entry<Handler>,

    pub debug: Entry<Handler>,

    pub non_maskable: Entry<Handler>,

    pub breakpoint: Entry<Handler>,

    pub overflow: Entry<Handler>,

    pub bound_range: Entry<Handler>,

    pub invalid_opcode: Entry<Handler>,

    pub device_not_available: Entry<Handler>,

    pub double_fault: Entry<DivergingCodeHandler>,

    segment_overrun: Entry<Handler>,

    pub invalid_tss: Entry<CodeHandler>,

    pub segment_not_present: Entry<CodeHandler>,

    pub stack: Entry<CodeHandler>,

    pub general_protection: Entry<CodeHandler>,

    pub page_fault: Entry<CodeHandler>,

    pub _reserved1: Entry<Handler>,

    pub x87_float_exception: Entry<Handler>,

    pub alignement_check: Entry<CodeHandler>,

    pub machine_check: Entry<DivergingCodeHandler>,

    pub simd_float: Entry<Handler>,

    pub virtualisation: Entry<Handler>,

    pub control_protection: Entry<CodeHandler>,
    _reserved2: [Entry<Handler>; 6],

    hypervisor_injection: Entry<Handler>, // amd64
    vmm_communication: Entry<Handler>,    // amd64
    security: Entry<Handler>,

    _reserved3: Entry<Handler>,

    pub user: [Entry<Handler>; 256 - 32],
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IdtPtr<'a> {
    limit: u16,
    addr: VirtualAddr,
    _m: PhantomData<&'a ()>,
}

pub fn lidt(ptr: &IdtPtr<'_>) {
    // SAFETY: we assure the IDT pointer is well defined
    unsafe {
        asm!("lidt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
    }
}

impl InterruptDescriptorTable {
    pub fn new() -> Self {
        Self {
            divide_by_zero: Entry::new(),
            debug: Entry::new(),
            non_maskable: Entry::new(),
            breakpoint: Entry::new(),
            overflow: Entry::new(),
            bound_range: Entry::new(),
            invalid_opcode: Entry::new(),
            device_not_available: Entry::new(),
            double_fault: Entry::new(),
            segment_overrun: Entry::new(),
            invalid_tss: Entry::new(),
            segment_not_present: Entry::new(),
            stack: Entry::new(),
            general_protection: Entry::new(),
            page_fault: Entry::new(),
            _reserved1: Entry::new(),
            x87_float_exception: Entry::new(),
            alignement_check: Entry::new(),
            machine_check: Entry::new(),
            simd_float: Entry::new(),
            virtualisation: Entry::new(),
            control_protection: Entry::new(),
            _reserved2: [Entry::new(); 6],
            hypervisor_injection: Entry::new(),
            vmm_communication: Entry::new(),
            security: Entry::new(),
            _reserved3: Entry::new(),
            user: [Entry::new(); 256 - 32],
        }
    }

    pub fn lidt_ptr(&self) -> IdtPtr<'_> {
        IdtPtr {
            limit: (core::mem::size_of::<Self>() - 1) as u16,
            addr: VirtualAddr::from_ptr(&self.divide_by_zero),
            _m: PhantomData,
        }
    }
}

impl Default for InterruptDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}

impl<H> core::ops::Deref for Entry<H> {
    type Target = InterruptGateDescriptor;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.igd
    }
}

impl<H> core::ops::DerefMut for Entry<H> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.igd
    }
}

impl<H> core::fmt::Debug for Entry<H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.igd.fmt(f)
    }
}
