#![no_std]

pub mod codec;
pub mod cursor;
pub mod read;
pub mod write;

#[cfg(test)]
extern crate std;

pub type Result<T> = core::result::Result<T, crate::Error>;

#[derive(Debug, Clone, Copy)]
pub struct Error {}

impl kcore::error::Error for Error {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
