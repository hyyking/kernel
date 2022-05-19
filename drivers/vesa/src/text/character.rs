use crate::{Drawable, Pixel};

use noto_sans_mono_bitmap::{get_bitmap, BitmapChar, BitmapHeight, FontWeight};

pub struct CharIter {
    bitmap: BitmapChar,
    from: (usize, usize),
    at: (usize, usize),
}

impl Iterator for CharIter {
    type Item = Pixel;

    fn next(&mut self) -> Option<Self::Item> {
        let (x, y) = self.from;

        if self.at.0 >= self.bitmap.width() {
            self.at.1 += 1;
            self.at.0 = 0;
        }
        let (xd, yd) = self.at;
        if yd >= self.bitmap.height() {
            return None;
        }
        let ret = Some(Pixel {
            x: x + xd,
            y: y + yd,
            intensity: self.bitmap.bitmap()[yd][xd],
        });
        self.at.0 += 1;

        ret
    }
}

pub struct Character {
    c: char,
    bitmap: BitmapChar,
    x: usize,
    y: usize,
}

impl Character {
    /// # Panics
    /// Panics if the character is not found in the bitmap
    #[must_use]
    pub fn new(c: char, x: usize, y: usize) -> Self {
        let bitmap = get_bitmap(c, FontWeight::Regular, BitmapHeight::Size14).unwrap();
        Self { c, bitmap, x, y }
    }

    #[inline]
    #[must_use]
    pub const fn height(&self) -> usize {
        self.bitmap.height()
    }

    #[inline]
    #[must_use]
    pub const fn width(&self) -> usize {
        self.bitmap.width()
    }
}

impl Drawable for Character {
    type Iter = CharIter;

    fn draw_box(&self) -> (usize, usize, usize, usize) {
        (
            self.x,
            self.y,
            self.x + self.bitmap.width(),
            self.y + self.bitmap.height(),
        )
    }

    fn points(&self) -> Self::Iter {
        CharIter {
            bitmap: get_bitmap(self.c, FontWeight::Regular, BitmapHeight::Size14).unwrap(),
            from: (self.x, self.y),
            at: (0, 0),
        }
    }
}
