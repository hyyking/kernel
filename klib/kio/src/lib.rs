#![no_std]

pub mod codec;
pub mod cursor;
pub mod read;
pub mod write;

#[cfg(test)]
extern crate std;

pub type Result<T> = core::result::Result<T, crate::Error>;

#[derive(Debug, Clone, Copy)]
pub struct Error {
    kind: ErrorKind,
 //   message: E
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum ErrorKind {
    StorageFull,
    OutOfMemory,
    InvalidData,
}

impl Error {
    #[inline]
    #[must_use]
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    #[inline]
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self {kind}
    }
}

impl kcore::error::Error for Error {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
