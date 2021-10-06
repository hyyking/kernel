use crate::words::{ICW1, ICW3, ICW4};
use crate::{Pic, Raw, RemapInit, RemapUninit};

use libx64::port::WPort;

enum State<const A: u8, const B: u8> {
    Init((Pic<RemapInit, A>, Pic<RemapInit, B>)),
    Uninit(Option<(Pic<RemapUninit, A>, Pic<RemapUninit, B>)>),
    Raw((Pic<Raw, A>, Pic<Raw, B>)),
}

pub struct ChainedPic<const A: u8, const B: u8> {
    state: State<A, B>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlreadyInit;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnhandledInterrupt;

impl<const A: u8, const B: u8> ChainedPic<A, B> {
    pub fn uninit() -> Self {
        Self {
            state: State::Uninit(Some((Pic::master(), Pic::slave()))),
        }
    }
    /// # Safety
    ///
    /// You must ensure the right usage for your pic
    pub unsafe fn raw(master: Pic<Raw, A>, slave: Pic<Raw, B>) -> Self {
        Self {
            state: State::Raw((master, slave)),
        }
    }

    pub fn init(&mut self) -> Result<(), AlreadyInit> {
        match self.state {
            State::Uninit(ref mut a @ Some(_)) => {
                let (master, slave) = a.take().unwrap();
                let masks = (master.read_mask(), slave.read_mask());
                self.state = State::Init(remap_init(master, slave, masks));
                Ok(())
            }
            State::Init(_) => Err(AlreadyInit),
            State::Raw(_) => Err(AlreadyInit),
            State::Uninit(None) => unreachable!("invalid ChainedPic state"),
        }
    }

    pub fn interupt_fn(&mut self, int_code: u8, f: impl Fn()) -> Result<(), UnhandledInterrupt> {
        let (master, slave) = match self.state {
            State::Init(ref mut m) => m,
            State::Uninit(_) => panic!("attempted to hande a "),
            State::Raw(_) => unimplemented!(""),
        };
        f();
        if master.handles_interrupt(int_code) || slave.handles_interrupt(int_code) {
            if slave.handles_interrupt(int_code) {
                slave.eoi();
            }
            master.eoi();
            Ok(())
        } else {
            Err(UnhandledInterrupt)
        }
    }
}

pub fn remap_init<const A: u8, const B: u8>(
    master: Pic<RemapUninit, A>,
    slave: Pic<RemapUninit, B>,
    masks: (u8, u8),
) -> (Pic<RemapInit, A>, Pic<RemapInit, B>) {
    let mut wait_port = WPort::<u8>::new(0x80);
    let mut wait = || unsafe { wait_port.write(0) };

    let mut icw1 = ICW1::zero();
    icw1.set_ic4(u8::from(true)).set_init(u8::from(true));

    let master = master.write_icw1(icw1);
    wait();
    let slave = slave.write_icw1(icw1);
    wait();

    let master = master.write_icw2();
    wait();
    let slave = slave.write_icw2();
    wait();

    let master = master.write_icw3(ICW3(4));
    wait();
    let slave = slave.write_icw3(ICW3(2));
    wait();

    let mut icw4 = ICW4::zero();
    icw4.set_x86mode(1);

    let master = master.write_icw4(icw4);
    wait();
    let slave = slave.write_icw4(icw4);
    wait();

    let (m1, m2) = masks;
    (master.write_mask(m1), slave.write_mask(m2))
}
