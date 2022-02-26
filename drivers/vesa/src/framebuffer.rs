use core::fmt;

use crate::{Drawable, Pixel};

use bootloader::boot_info::{FrameBufferInfo, PixelFormat};
use kcore::ptr::volatile::Volatile;

pub struct Framebuffer {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
}

impl Framebuffer {
    /// Creates a new logger that uses the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut logger = Self {
            framebuffer,
            info,
            x_pos: 0,
            y_pos: 0,
        };
        logger.clear();
        logger
    }

    pub fn draw<I>(&mut self, obj: &dyn Drawable<Iter = I>) -> Result<(), ()>
    where
        I: Iterator<Item = Pixel>,
    {
        let (x_t, y_t, x_b, y_b) = obj.draw_box();
        if x_b >= self.info.horizontal_resolution && y_b >= self.info.vertical_resolution {
            return Err(());
        }

        let pad = obj.padding();

        let (x_t, y_t, x_b, y_b) = (x_t + pad, y_t + pad, x_b - pad, y_b - pad);

        obj.points().try_for_each(|Pixel { x, y, intensity }| {
            if x >= x_t && y >= y_t && x <= x_b && y <= y_b {
                Ok(self.write_pixel(x, y, intensity))
            } else {
                Err(())
            }
        })
    }

    /// Erases all text on the screen.
    pub fn clear(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;
        self.framebuffer.fill(0);
    }

    pub fn width(&self) -> usize {
        self.info.horizontal_resolution
    }

    pub fn height(&self) -> usize {
        self.info.vertical_resolution
    }
    /*
        pub fn newline(&mut self) {
            self.y_pos += 14 + LINE_SPACING;
            self.carriage_return()
        }

        pub fn add_vspace(&mut self, space: usize) {
            self.y_pos += space;
        }

        pub fn carriage_return(&mut self) {
            self.x_pos = 0;
        }
        pub fn write_char(&mut self, c: char) {
            match c {
                '\n' => self.newline(),
                '\r' => self.carriage_return(),
                c => {
                    if self.x_pos >= self.width() {
                        self.newline();
                    }
                    const BITMAP_LETTER_WIDTH: usize =
                        get_bitmap_width(FontWeight::Regular, BitmapHeight::Size14);
                    if self.y_pos >= (self.height() - BITMAP_LETTER_WIDTH) {
                        self.clear();
                    }
                    let bitmap_char = get_bitmap(c, FontWeight::Regular, BitmapHeight::Size14).unwrap();
                    self.write_rendered_char(bitmap_char);
                }
            }
        }

        fn write_rendered_char(&mut self, rendered_char: BitmapChar) {
            for (y, row) in rendered_char.bitmap().iter().enumerate() {
                for (x, byte) in row.iter().enumerate() {
                    self.write_pixel(self.x_pos + x, self.y_pos + y, *byte);
                }
            }
            self.x_pos += rendered_char.width();
        }
    */

    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = y * self.info.stride + x;
        let color = match self.info.pixel_format {
            PixelFormat::RGB => [intensity, intensity, intensity / 2, 0],
            PixelFormat::BGR => [intensity / 2, intensity, intensity, 0],
            PixelFormat::U8 => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
            _ => unimplemented!(),
        };
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
        let _ = Volatile::new(&self.framebuffer[byte_offset]).read();
    }
}

unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}

impl fmt::Write for Framebuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.chars().try_for_each(|c| self.write_char(c))
    }
}
