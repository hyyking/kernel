#![feature(abi_x86_interrupt)]
#![allow(dead_code)]

#[repr(C)]
pub struct InterruptFrame {
    a: u64,
}

#[interrupt_list::interrupt_list(InterruptListStruct)]
pub mod a {
    use super::*;

    const TEST_CONST: () = ();

    static TEST_STATIC: Option<()> = None;
    static mut TEST_STATIC_MUT: Option<()> = None;

    #[interrupt_list::user_interrupt(35)]
    pub extern "x86-interrupt" fn foo_test(_f: InterruptFrame) {}

    #[interrupt_list::user_interrupt(33)]
    pub extern "x86-interrupt" fn bar(_f: InterruptFrame) {}

    #[interrupt_list::user_interrupt(34)]
    pub extern "x86-interrupt" fn timerr(_f: InterruptFrame) {}

    #[interrupt_list::user_interrupt(32)]
    pub extern "x86-interrupt" fn timer(_f: InterruptFrame) {}
}

fn main() {}
