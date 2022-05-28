#![no_std]

#[cfg(test)]
extern crate std;

pub struct CobsCodec;

impl kio::codec::Encoder<&[u8]> for CobsCodec {
    type Error = kio::Error;

    fn encode<T>(&mut self, item: &[u8], mut dst: T) -> Result<usize, Self::Error>
    where
        T: AsMut<[u8]>,
    {
        let n = encode(item, dst.as_mut())?;
        dst.as_mut()[n] = 0;
        Ok(n + 1)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct EncoderState {
    cursor_buffer: usize,
    code: u8,
    cursor_code: usize,
    n: usize,
}

impl EncoderState {
    const fn new() -> Self {
        Self {
            cursor_buffer: 1,
            code: 1,
            cursor_code: 0,
            n: 1,
        }
    }
}

struct FramedEncoder<T>
where
    T: AsMut<[u8]>,
{
    state: EncoderState,
    buffer: T,
}

impl<T> FramedEncoder<T>
where
    T: AsMut<[u8]>,
{
    fn encode_byte(self, byte: u8) -> Result<Self, kio::Error> {
        let Self {
            mut state,
            buffer: mut buf,
        } = self;
        let buffer = buf.as_mut();

        if state.cursor_buffer >= buffer.len() {
            return Err(kio::Error::new(kio::ErrorKind::StorageFull));
        }

        if byte != 0 {
            buffer[state.cursor_buffer] = byte;
            state.code += 1;
            state.cursor_buffer += 1;
        }
        if byte == 0 || state.code == 0xFF {
            buffer[state.cursor_code] = state.code;
            state.code = 1;
            state.cursor_code = state.cursor_buffer;
            state.cursor_buffer += 1;
        }

        state.n += 1;
        Ok(Self { state, buffer: buf })
    }

    fn finish(mut self) -> (usize, T) {
        self.buffer.as_mut()[self.state.cursor_code] = self.state.code;
        (self.state.n, self.buffer)
    }
}

impl<T> From<T> for FramedEncoder<T>
where
    T: AsMut<[u8]>,
{
    #[inline]
    fn from(buffer: T) -> Self {
        Self {
            state: EncoderState::new(),
            buffer,
        }
    }
}

/// Encodes without the terminator
///
/// # Errors
///
/// Errors if the buffer is not large enough
pub fn encode(input: &[u8], buffer: &mut [u8]) -> Result<usize, kio::Error> {
    Ok(input
        .iter()
        .copied()
        .try_fold(FramedEncoder::from(buffer), FramedEncoder::encode_byte)?
        .finish()
        .0)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct DecoderState {
    cursor_buffer: usize,
    n: usize,
    block: u8,
    code: u8,
}

impl DecoderState {
    const fn new() -> Self {
        Self {
            cursor_buffer: 0,
            code: 0xFF,
            block: 0,
            n: 0,
        }
    }
}

struct FramedDecoder<T>
where
    T: AsMut<[u8]>,
{
    state: DecoderState,
    buffer: T,
}

impl<T> FramedDecoder<T>
where
    T: AsMut<[u8]>,
{
    fn decode_byte(self, byte: u8) -> Self {
        let Self {
            mut state,
            mut buffer,
        } = self;
        if state.block == 0 {
            if state.code != 0xFF {
                buffer.as_mut()[state.cursor_buffer] = 0;
                state.cursor_buffer += 1;
                state.n += 1;
            }
            state.code = byte;
            state.block = byte;
        } else {
            buffer.as_mut()[state.cursor_buffer] = byte;
            state.cursor_buffer += 1;
            state.n += 1;
        }
        state.block -= 1;
        Self { state, buffer }
    }

    fn finish(self) -> (usize, T) {
        (self.state.n, self.buffer)
    }
}

impl<T> From<T> for FramedDecoder<T>
where
    T: AsMut<[u8]>,
{
    #[inline]
    fn from(buffer: T) -> Self {
        Self {
            state: DecoderState::new(),
            buffer,
        }
    }
}

/// The input must be zero delimited
pub fn decode(input: &[u8], buffer: &mut [u8]) -> usize {
    input
        .iter()
        .copied()
        .take_while(|&n| n != 0)
        .fold(FramedDecoder::from(buffer), FramedDecoder::decode_byte)
        .finish()
        .0
}

#[cfg(test)]
mod tests {

    #[rustfmt::skip]
    static PYTHON_COBS: &[[&[u8]; 2]] = &[
        [b""                    as &[u8], b"\x01"                   as &[u8]],
        [b"1"                   as &[u8], b"\x021"                  as &[u8]],
        [b"12345"               as &[u8], b"\x0612345"              as &[u8]],
        [b"12345\x006789"       as &[u8], b"\x0612345\x056789"      as &[u8]],
        [b"\x0012345\x006789"   as &[u8], b"\x01\x0612345\x056789"  as &[u8]],
        [b"12345\x006789\x00"   as &[u8], b"\x0612345\x056789\x01"  as &[u8]],
        [b"\x00"                as &[u8], b"\x01\x01"               as &[u8]],
        [b"\x00\x00"            as &[u8], b"\x01\x01\x01"           as &[u8]],
        [b"\x00\x00\x00"        as &[u8], b"\x01\x01\x01\x01"       as &[u8]],
    ];

    #[rustfmt::skip]
    static WIKIPEDIA_COBS: &[[&[u8];2]] = &[
      [&[00            ], &[01, 01,           ]],
      [&[00, 00        ], &[01, 01, 01,       ]],
      [&[00, 11, 00    ], &[01, 02, 11, 01,   ]],
      [&[11, 22, 00, 33], &[03, 11, 22, 02, 33]],
      [&[11, 22, 33, 44], &[05, 11, 22, 33, 44]],
      [&[11, 00, 00, 00], &[02, 11, 01, 01, 01]],
    ];

    mod decode {
        use super::*;
        use crate::decode;
        use std::vec::Vec;

        #[test]
        fn python_cobs() {
            // https://github.com/cmcqueen/cobs-python/blob/main/python3/cobs/cobs/test.py
            let mut buffer = [0u8; 256];
            for [output, input] in PYTHON_COBS {
                let mut input = Vec::from(*input);
                input.push(0);
                let n = decode(&input[..], &mut buffer);
                assert_eq!(&&buffer[..n], output);
            }
        }

        #[test]
        fn wikipedia_cobs() {
            // https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing

            let mut buffer = [0u8; 256];
            for [output, input] in WIKIPEDIA_COBS {
                let mut input = Vec::from(*input);
                input.push(0);
                let n = decode(&input[..], &mut buffer);
                assert_eq!(&&buffer[..n], output);
            }
        }
    }

    mod encode {
        use super::*;
        use crate::encode;

        #[test]
        fn python_cobs() {
            // https://github.com/cmcqueen/cobs-python/blob/main/python3/cobs/cobs/test.py
            let mut buffer = [0u8; 256];
            for [input, output] in PYTHON_COBS {
                let n = encode(input, &mut buffer).unwrap();
                assert_eq!(&&buffer[..n], output);
            }
        }

        #[test]
        fn wikipedia_cobs() {
            // https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing

            let mut buffer = [0u8; 256];
            for [input, output] in WIKIPEDIA_COBS {
                let n = encode(input, &mut buffer).unwrap();
                assert_eq!(&&buffer[..n], output);
            }
        }

        #[test]
        fn empty_no_zero() {
            let mut buffer = [0u8; 256];

            let n = encode(&[2; 32][..], &mut buffer).unwrap();
            assert_eq!(n, 33); // + one delimiter
            assert_eq!(buffer[0], 33);
            assert_eq!(&buffer[1..n], &[2; 32][..]);
        }

        #[test]
        fn full_zero() {
            let mut buffer = [0u8; 1024];

            let n = encode(&[00; 256][..], &mut buffer).unwrap();
            assert_eq!(n, 257); // + one delimiter
            assert_eq!(&buffer[..n], &[1; 512][..n]);

            let n = encode(&[00; 512][..], &mut buffer).unwrap();
            assert_eq!(n, 513); // + one delimiter
            assert_eq!(&buffer[..n], &[1; 1024][..n]);
        }

        #[test]
        fn full_nonzero() {
            let mut buffer = [0u8; 1024];
            let n = encode(&[1; 512][..], &mut buffer).unwrap();

            // 512/255 = 2, 512 % 255 = 2
            // so we have two full blocks + one trailing + 2 bytes of trailing data
            assert_eq!(n, 0xFF * 2 + 1 + 2);

            // first block validation
            assert_eq!(buffer[0], 0xFF);
            assert_eq!(&buffer[1..0xFF], &[1; 0xFE][..]);

            // second block validation
            assert_eq!(buffer[0xFF], 0xFF);
            assert_eq!(&buffer[0x100..0x1FE], &[1; 0xFE][..]);

            // third block validation: 3 overhead bytes and the remainder of the data
            assert_eq!(buffer[0x1FE] as usize, 1 + 1 + 1 + (512 % 0xFF));

            assert_eq!(
                &buffer[0x1FF..(0x1FF + 1 + 1 + (512 % 0xFF))],
                &[1; 1 + 1 + (512 % 0xFF)][..]
            );
        }
    }
}
