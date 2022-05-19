use core::ptr::NonNull;

use crate::{text::character::CharIter, text::Character, Drawable, Pixel};

use noto_sans_mono_bitmap::{get_bitmap, BitmapHeight, FontWeight};

const LETTER_PAD: usize = 0;

pub struct Text<A> {
    text: A,
    at: (usize, usize),
}

impl<A> Text<A>
where
    A: AsRef<str>,
{
    pub fn new(text: A, x: usize, y: usize) -> Self {
        Self { text, at: (x, y) }
    }
}

impl<A> Drawable for Text<A>
where
    A: AsRef<str>,
{
    type Iter = TextIter;

    fn draw_box(&self) -> (usize, usize, usize, usize) {
        let height = self
            .text
            .as_ref()
            .chars()
            .map(|c| {
                get_bitmap(c, FontWeight::Regular, BitmapHeight::Size14)
                    .unwrap()
                    .height()
            })
            .max()
            .unwrap_or(0);
        let width: usize = self
            .text
            .as_ref()
            .chars()
            .map(|c| {
                get_bitmap(c, FontWeight::Regular, BitmapHeight::Size14)
                    .unwrap()
                    .width()
                    + LETTER_PAD
            })
            .sum();
        let (x, y) = self.at;
        (x, y, x + width, y + height)
    }

    fn points(&self) -> Self::Iter {
        TextIter {
            text: NonNull::from(self.text.as_ref()),
            at: self.at,
            idx: 0,
            curr: None,
        }
    }
}

pub struct TextIter {
    text: NonNull<str>,
    at: (usize, usize),
    idx: usize,
    curr: Option<CharIter>,
}

impl Iterator for TextIter {
    type Item = Pixel;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(curr) = self.curr.as_mut() {
                match curr.next() {
                    Some(pixel) => return Some(pixel),
                    None => {
                        self.at.0 += LETTER_PAD;
                    }
                }
            }

            let c = unsafe { self.text.as_ref().chars().nth(self.idx + 1)? };
            let c = Character::new(c, self.at.0, self.at.1);
            self.at.0 += c.width();
            self.curr = Some(c.points());
            self.idx += 1;
        }
    }
}
