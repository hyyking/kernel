use std::io;

use bytes::{Bytes, BytesMut};
use rkyv::{util, ArchiveUnsized};

use protocols::log::LogPacket;

pub struct LogRef(pub Bytes);
impl AsRef<<LogPacket<'static> as ArchiveUnsized>::Archived> for LogRef {
    fn as_ref(&self) -> &<LogPacket<'static> as ArchiveUnsized>::Archived {
        unsafe { util::archived_unsized_root::<LogPacket<'static>>(&self.0[..]) }
    }
}

pub struct LogDecoder;

impl LogDecoder {
    #[rustfmt::skip]
    pub const fn new() -> Self { Self }
}

impl tokio_util::codec::Decoder for LogDecoder {
    type Item = LogRef;

    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let buf = match src
            .iter()
            .enumerate()
            .find_map(|(i, &b)| (b == 0).then_some(i + 1))
        {
            Some(n) => {
                let buf = src.split_to(n);
                src.reserve(1024);
                buf.freeze()
            }
            None => return Ok(None),
        };

        let mut decode = BytesMut::new();
        decode.resize(buf.len(), 0);
        let n = mais::decode(&buf[..], &mut decode[..]);
        decode.truncate(n);

        Ok(Some(LogRef(decode.freeze())))
    }
}
