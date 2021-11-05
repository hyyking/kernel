#![feature(abi_x86_interrupt)]
#![allow(dead_code)]

extern crate interrupt_list;

#[repr(C)]
pub struct InterruptFrame {
    a: u64,
}

#[interrupt_list::interrupt_list(InterruptListStruct)]
pub mod a {
    use super::*;

    pub const TEST_CONST: () = ();

    pub static TEST_STATIC: Option<()> = None;
    pub static mut TEST_STATIC_MUT: Option<()> = Some(());

    #[interrupt_list::user_interrupt(35)]
    pub extern "x86-interrupt" fn foo_test(_f: InterruptFrame) {}

    #[interrupt_list::user_interrupt(33)]
    pub extern "x86-interrupt" fn bar(_f: InterruptFrame) {}

    #[interrupt_list::user_interrupt(34)]
    pub extern "x86-interrupt" fn timerr(_f: InterruptFrame) {}

    #[interrupt_list::user_interrupt(32)]
    pub extern "x86-interrupt" fn timer(_f: InterruptFrame) {}
}

fn main() {
    assert_eq!(a::InterruptListStruct::FooTest as u8, 35);
    assert_eq!(a::InterruptListStruct::Bar as u8, 35);
    assert_eq!(a::InterruptListStruct::Timerr as u8, 34);
    assert_eq!(a::InterruptListStruct::Timer as u8, 32);

    assert_eq!(a::TEST_CONST, ());
    assert_eq!(a::TEST_STATIC, None);
    assert_eq!(unsafe { a::TEST_STATIC_MUT.take() }, Some(()));
}
