#![no_std]

pub mod framebuffer;
pub mod text;

#[derive(Debug, Clone, Copy)]
pub struct Pixel {
    x: usize,
    y: usize,
    intensity: u8,
}

pub trait Drawable {
    type Iter: Iterator<Item = Pixel>;

    fn draw_box(&self) -> (usize, usize, usize, usize);
    fn points(&self) -> Self::Iter;
    fn padding(&self) -> usize {
        0
    }
}
