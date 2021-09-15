use core::fmt::{self, Write};

use kcore::{ptr::volatile::Volatile, sync::mutex::SpinMutex};

klazy! {
    pub ref static DRIVER: SpinMutex<VgaDriver<80, 25>> = SpinMutex::new(VgaDriver::new());
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::drivers::vga::_kprint(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("\n"));
    ($($arg:tt)*) => ($crate::kprint!("{}\n", format_args!($($arg)*)));
}

pub fn _kprint(args: fmt::Arguments) {
    DRIVER.lock().cursor().write_fmt(args).expect("kprint");
}

#[allow(dead_code)]
#[repr(u8)]
pub enum Color {
    Black = 0x00,
    Blue = 0x01,
    Green = 0x02,
    Cyan = 0x03,
    Red = 0x04,
    Magenta = 0x05,
    Brown = 0x6,
    White = 0x07,
    Gray = 0x08,
    LightBlue = 0x09,
    LightGreen = 0x0A,
    LightCyan = 0x0B,
    LightRed = 0x0C,
    LightMagenta = 0x0D,
    Yellow = 0x0E,
    BrightWhite = 0x0F,
}

#[repr(C)]
pub struct Character {
    ccode: u8,
    color: u8,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
struct Point<T> {
    x: T,
    y: T,
}

pub struct VgaDriver<const C: usize, const L: usize> {
    buffer: &'static mut [[Character; C]; L],
    position: Point<usize>,
}

pub struct Cursor<'a, const C: usize, const L: usize> {
    driver: &'a mut VgaDriver<C, L>,
}

impl Character {
    pub fn set_char(&mut self, c: char) {
        self.ccode = c as u8;
    }
    pub fn set_color(&mut self, fg: Color, bg: Color) {
        let fg = fg as u8 & 0b0000_1111; // mask unused bytes
        let bg = bg as u8 & 0b0000_1111; // mask unused bytes
        self.color = fg | (bg << 4);
    }
}

impl<const C: usize, const L: usize> VgaDriver<C, L> {
    const TAB_SIZE: usize = 4;

    pub fn new() -> Self {
        Self {
            buffer: unsafe { core::mem::transmute(0xb8000 as *mut Self) },
            position: Point { x: 0, y: 0 },
        }
    }

    pub fn cursor(&mut self) -> Cursor<'_, C, L> {
        Cursor { driver: self }
    }
}

impl<'a, const C: usize, const L: usize> Cursor<'a, C, L> {
    pub fn character_mut(&mut self) -> Volatile<&mut Character> {
        let Point { x, y } = self.driver.position;
        Volatile::new(&mut self.driver.buffer[y][x])
    }

    pub fn write_character(&mut self, c: char, fg: Color, bg: Color) -> core::fmt::Result {
        if !c.is_ascii() {
            return Err(core::fmt::Error);
        }
        match c {
            '\n' => {
                self.driver.position.x += C;
                self.next_pos();
            }
            '\t' => {
                let m = self.driver.position.x % VgaDriver::<C, L>::TAB_SIZE;
                self.driver.position.x += VgaDriver::<C, L>::TAB_SIZE - m;
                self.next_pos();
            }
            _ => {
                self.character_mut().update(|old| {
                    old.set_char(c);
                    old.set_color(fg, bg);
                });

                self.next_pos();
            }
        }
        Ok(())
    }

    pub fn next_pos(&mut self) {
        match self.driver.position {
            Point { x, .. } if x + 1 >= C => {
                self.driver.position.y += 1;
                self.driver.position.x = 0;
            }
            _ => {
                self.driver.position.x += 1;
            }
        }
    }
}

impl<'a, const C: usize, const L: usize> Write for Cursor<'a, C, L> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().try_for_each(|c| self.write_char(c))
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.write_character(c, Color::BrightWhite, Color::Black)?;
        Ok(())
    }
}
