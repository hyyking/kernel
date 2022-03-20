use std::io;

use bytes::BytesMut;
use rkyv::{util, Archive};

use protocols::log::{LogHeader, LogMessage, HEADER_SIZE};

enum DecoderState {
    WaitingForHeader,
    ReadingMessage(usize),
}

pub struct LogDecoder {
    state: DecoderState,
}

impl LogDecoder {
    pub const fn new() -> Self {
        Self {
            state: DecoderState::WaitingForHeader,
        }
    }

    pub fn decode_ref<'a>(
        &mut self,
        src: &'a mut BytesMut,
    ) -> Result<Option<&'a <LogMessage as Archive>::Archived>, io::Error> {
        match self.state {
            DecoderState::WaitingForHeader => {
                if src.len() < HEADER_SIZE {
                    return Ok(None);
                }
                let header =
                    match unsafe { rkyv::from_bytes_unchecked::<LogHeader>(&src[..HEADER_SIZE]) } {
                        Ok(header) => header,
                        Err(_) => return Ok(None),
                    };
                if header.size == 0 {
                    return Ok(None);
                }

                src.reserve(HEADER_SIZE + header.size);
                drop(src.split_to(HEADER_SIZE));

                self.state = DecoderState::ReadingMessage(header.size);
                return Ok(None);
            }
            DecoderState::ReadingMessage(n) => {
                if src.len() >= n {
                    let message = unsafe { util::archived_unsized_root::<LogMessage>(&src[..n]) };
                    self.state = DecoderState::WaitingForHeader;
                    return Ok(Some(message));
                }
                return Ok(None);
            }
        }
    }
}
