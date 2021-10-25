#![no_std]
#![feature(marker_trait_attr)]

use libx64::port::RWPort;

pub mod chained;
pub mod words;

use words::{ICW1, ICW3, ICW4};

#[marker]
pub trait PicState {}
#[marker]
pub trait PicFinalState: PicState {}
#[marker]
pub trait PicStartState: PicState {}

pub struct Raw;
impl PicState for Raw {}
impl PicFinalState for Raw {}
impl PicStartState for Raw {}

pub struct RemapUninit;
impl PicState for RemapUninit {}
impl PicStartState for RemapUninit {}

struct RemapICW1;
impl PicState for RemapICW1 {}
struct RemapICW2;
impl PicState for RemapICW2 {}
struct RemapICW3;
impl PicState for RemapICW3 {}
struct RemapICW4;
impl PicState for RemapICW4 {}
pub struct RemapInit;
impl PicState for RemapInit {}
impl PicFinalState for RemapInit {}

/// # Programmable Interupt Controller
///
/// Source: <http://www.brokenthorn.com/Resources/OSDevPic.html>
///
/// ## 8259A Software Port Map
///
/// Port Address | Description                                                    
/// -------------|----------------------------------------------------------------
/// 0x20         | Primary PIC Command and Status Register                        
/// 0x21         | Primary PIC Interrupt Mask Register and Data Register          
/// 0xA0         | Secondary (Slave) PIC Command and Status Register              
/// 0xA1         | Secondary (Slave) PIC Interrupt Mask Register and Data Register
pub struct Pic<S: PicState, const OFFSET: u8> {
    command: RWPort<u8>,
    data: RWPort<u8>,
    _s: core::marker::PhantomData<S>,
}

impl<S, const OFFSET: u8> Pic<S, OFFSET>
where
    S: PicStartState,
{
    /// # Safety
    ///
    /// You must assure the offset are within the IDT range
    pub unsafe fn new(command: u16, data: u16) -> Self {
        Self {
            command: RWPort::new(command),
            data: RWPort::new(data),
            _s: core::marker::PhantomData,
        }
    }

    pub fn master() -> Self {
        unsafe { Self::new(0x20, 0x21) }
    }

    pub fn slave() -> Self {
        unsafe { Self::new(0xA0, 0xA1) }
    }

    pub fn read_mask(&self) -> u8 {
        unsafe { self.data.read() }
    }
}

impl<const OFFSET: u8> Pic<RemapUninit, OFFSET> {
    fn write_icw1(self, icw1: ICW1) -> Pic<RemapICW1, OFFSET> {
        let Self {
            mut command, data, ..
        } = self;
        unsafe {
            command.write(icw1.as_u8());
        }
        Pic {
            command,
            data,
            _s: core::marker::PhantomData,
        }
    }
}

impl<const OFFSET: u8> Pic<RemapICW1, OFFSET> {
    fn write_icw2(self) -> Pic<RemapICW2, OFFSET> {
        let Self {
            command, mut data, ..
        } = self;
        unsafe {
            data.write(OFFSET);
        }
        Pic {
            command,
            data,
            _s: core::marker::PhantomData,
        }
    }
}

impl<const OFFSET: u8> Pic<RemapICW2, OFFSET> {
    fn write_icw3(self, icw3: ICW3) -> Pic<RemapICW3, OFFSET> {
        let Self {
            command, mut data, ..
        } = self;
        unsafe {
            data.write(icw3.0);
        }
        Pic {
            command,
            data,
            _s: core::marker::PhantomData,
        }
    }
}

impl<const OFFSET: u8> Pic<RemapICW3, OFFSET> {
    fn write_icw4(self, icw4: ICW4) -> Pic<RemapICW4, OFFSET> {
        let Self {
            command, mut data, ..
        } = self;
        unsafe {
            data.write(icw4.as_u8());
        }
        Pic {
            command,
            data,
            _s: core::marker::PhantomData,
        }
    }
}

impl<const OFFSET: u8> Pic<RemapICW4, OFFSET> {
    fn write_mask(self, mask: u8) -> Pic<RemapInit, OFFSET> {
        let Self {
            command, mut data, ..
        } = self;
        unsafe {
            data.write(mask);
        }
        Pic {
            command,
            data,
            _s: core::marker::PhantomData,
        }
    }
}

impl<S, const OFFSET: u8> Pic<S, OFFSET>
where
    S: PicFinalState,
{
    const EOI: u8 = 0x20;
    pub fn eoi(&mut self) {
        unsafe {
            self.command.write(Self::EOI);
        }
    }
}

impl<S: PicState, const OFFSET: u8> Pic<S, OFFSET> {
    // End of interupt command

    pub fn handles_interrupt(&self, id: u8) -> bool {
        OFFSET <= id && id < OFFSET + 8
    }
}
