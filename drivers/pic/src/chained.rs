use crate::words::{ICW1, ICW3, ICW4};
use crate::{Pic, Raw, RemapInit, RemapUninit};

use libx64::port::WPort;

enum State<const A: u8, const B: u8> {
    Init((Pic<RemapInit, A>, Pic<RemapInit, B>)),
    Uninit(Option<(Pic<RemapUninit, A>, Pic<RemapUninit, B>)>),
    Raw((Pic<Raw, A>, Pic<Raw, B>)),
}

pub struct Chained<const A: u8, const B: u8> {
    state: State<A, B>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    UnhandledInterrupt,
    AlreadyInit,
    UnexpectedUnitialized,
}

impl<const A: u8, const B: u8> Chained<A, B> {
    #[must_use]
    pub fn uninit() -> Self {
        Self {
            state: State::Uninit(Some((Pic::master(), Pic::slave()))),
        }
    }
    /// # Safety
    ///
    /// You must ensure the right usage for your pic
    #[must_use]
    pub unsafe fn raw(master: Pic<Raw, A>, slave: Pic<Raw, B>) -> Self {
        Self {
            state: State::Raw((master, slave)),
        }
    }

    /// # Errors
    ///
    /// Errors if the pic is initialized
    pub fn init(&mut self) -> Result<(), Error> {
        match self.state {
            State::Uninit(Some((ref mut master, ref mut slave))) => {
                let masks = (master.read_mask(), slave.read_mask());
                self.state = State::Init(remap_init(master.clone(), slave.clone(), masks));
                Ok(())
            }
            State::Init(_) | State::Raw(_) => Err(Error::AlreadyInit),
            State::Uninit(None) => Err(Error::UnexpectedUnitialized),
        }
    }

    /// # Errors
    ///
    /// This function errors if the chained pic doesn't handle this interrupt, or isn't intialized
    ///
    /// # Panics
    /// TODO: implement `State::Raw`
    pub fn interupt_fn<T>(&mut self, int_code: T) -> Result<(), Error>
    where
        T: libx64::idt::TrustedUserInterruptIndex,
    {
        let (master, slave) = match self.state {
            State::Init(ref mut m) => m,
            State::Uninit(_) => return Err(Error::UnexpectedUnitialized),
            State::Raw(_) => unimplemented!(),
        };
        let int_code: u8 = u8::try_from(int_code.into()).unwrap();
        if master.handles_interrupt(int_code) || slave.handles_interrupt(int_code) {
            if slave.handles_interrupt(int_code) {
                slave.eoi();
            }
            master.eoi();
            Ok(())
        } else {
            Err(Error::UnhandledInterrupt)
        }
    }
}

#[must_use]
pub fn remap_init<const A: u8, const B: u8>(
    master: Pic<RemapUninit, A>,
    slave: Pic<RemapUninit, B>,
    masks: (u8, u8),
) -> (Pic<RemapInit, A>, Pic<RemapInit, B>) {
    let mut wait_port = WPort::<u8>::new(0x80);
    let mut wait = || unsafe { wait_port.write(0) };

    let icw1 = ICW1::zero()
        .set_ic4(u8::from(true))
        .set_init(u8::from(true));

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

    let icw4 = ICW4::zero().set_x86mode(1);

    let master = master.write_icw4(icw4);
    wait();
    let slave = slave.write_icw4(icw4);
    wait();

    let (m1, m2) = masks;
    (master.write_mask(m1), slave.write_mask(m2))
}
