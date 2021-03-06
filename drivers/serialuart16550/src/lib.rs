#![no_std]

use bitflags::bitflags;

use libx64::port::{Port, RPort, RWPort, WPort};

bitflags! {
    /// Interrupt enable flags
    struct IntEnFlags: u8 {
        const RECEIVED = 1;
        const SENT = 1 << 1;
        const ERRORED = 1 << 2;
        const STATUS_CHANGE = 1 << 3;
    }
}

bitflags! {
    /// Line status flags
    struct LineStsFlags: u8 {
        const INPUT_FULL = 1;
        // 1 to 4 unknown
        const OUTPUT_EMPTY = 1 << 5;
        // 6 and 7 unknown
    }
}

macro_rules! wait_for {
    ($cond:expr) => {
        #[allow(clippy::semicolon_if_nothing_returned)]
        while (!$cond) {
            core::hint::spin_loop()
        }
    };
}

#[derive(Debug)]
pub struct SerialPort {
    data: RWPort<u8>,
    int_en: WPort<u8>,
    fifo_ctrl: WPort<u8>,
    line_ctrl: WPort<u8>,
    modem_ctrl: WPort<u8>,
    line_sts: RPort<u8>,
}

impl SerialPort {
    /// Creates a new serial port interface on the given I/O port.
    ///
    /// # Safety
    /// This function is unsafe because the caller must ensure that the given base address
    /// really points to a serial port device.
    #[must_use]
    pub const unsafe fn new(base: u16) -> Self {
        Self {
            data: Port::new(base),
            int_en: WPort::new(base + 1),
            fifo_ctrl: WPort::new(base + 2),
            line_ctrl: WPort::new(base + 3),
            modem_ctrl: WPort::new(base + 4),
            line_sts: RPort::new(base + 5),
        }
    }

    /// Initializes the serial port.
    ///
    /// The default configuration of [38400/8-N-1](https://en.wikipedia.org/wiki/8-N-1) is used.
    pub fn init(&mut self) {
        unsafe {
            // Disable interrupts
            self.int_en.write(0x00);

            // Enable DLAB
            self.line_ctrl.write(0x80);

            // Set maximum speed to 38400 bps by configuring DLL and DLM
            self.data.write(0x03);
            self.int_en.write(0x00);

            // Disable DLAB and set data word length to 8 bits
            self.line_ctrl.write(0x03);

            // Enable FIFO, clear TX/RX queues and
            // set interrupt watermark at 14 bytes
            self.fifo_ctrl.write(0xC7);

            // Mark data terminal ready, signal request to send
            // and enable auxilliary output #2 (used as interrupt line for CPU)
            self.modem_ctrl.write(0x0B);

            // Enable interrupts
            self.int_en.write(0x01);
        }
    }

    fn line_sts(&mut self) -> LineStsFlags {
        unsafe { LineStsFlags::from_bits_truncate(self.line_sts.read()) }
    }

    /// Sends a byte on the serial port.
    pub fn send_char(&mut self, c: char) {
        assert!(char::is_ascii(&c));
        unsafe {
            match c as u8 {
                8 | 0x7F => {
                    wait_for!(self.line_sts().contains(LineStsFlags::OUTPUT_EMPTY));
                    self.data.write(8);
                    wait_for!(self.line_sts().contains(LineStsFlags::OUTPUT_EMPTY));
                    self.data.write(b' ');
                    wait_for!(self.line_sts().contains(LineStsFlags::OUTPUT_EMPTY));
                    self.data.write(8);
                }
                data => {
                    wait_for!(self.line_sts().contains(LineStsFlags::OUTPUT_EMPTY));
                    self.data.write(data);
                }
            }
        }
    }

    pub fn send_raw(&mut self, data: u8) {
        unsafe {
            wait_for!(self.line_sts().contains(LineStsFlags::OUTPUT_EMPTY));
            self.data.write(data);
        }
    }

    /// Receives a byte on the serial port.
    pub fn receive(&mut self) -> u8 {
        unsafe {
            wait_for!(self.line_sts().contains(LineStsFlags::INPUT_FULL));
            self.data.read()
        }
    }
}

impl kio::write::Write for SerialPort {
    fn write(&mut self, buffer: &[u8]) -> kio::Result<usize> {
        buffer.iter().copied().for_each(|b| self.send_raw(b));
        Ok(buffer.len())
    }

    fn write_all(&mut self, buffer: &[u8]) -> kio::Result<()> {
        buffer.iter().copied().for_each(|b| self.send_raw(b));
        Ok(())
    }
}

impl core::fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().into_iter().for_each(|b| self.send_char(b));
        Ok(())
    }
}
