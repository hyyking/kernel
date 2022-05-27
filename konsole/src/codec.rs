use std::io;

use bytes::BytesMut;
use rkyv::{util, AlignedVec, ArchiveUnsized};

use protocols::log::LogPacket;

pub struct LogRef(pub AlignedVec);
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

        let mut decode = rkyv::AlignedVec::with_capacity(buf.len());
        unsafe { decode.set_len(buf.len()) };
        decode.fill(0);
        let n = mais::decode(&buf[..], &mut decode[..]);

        unsafe { decode.set_len(n) };
        Ok(Some(LogRef(decode)))
    }
}

#[test]
fn deser() {
    use bytes::BufMut;
    use protocols::log::{ArchivedLogPacket, LogPacket, Span};
    use rkyv::ser::Serializer;
    use tokio_util::codec::Decoder;

    let p = LogPacket::NewSpan(Span {
        id: 8,
        target: "map_range",
        fields: "pages=PageRange<2Mb>(0x1000000000..0x1100000000),frames=FrameRange<2Mb>(0x0..0x100000000),flags=3,",
    });
    // let a = rkyv::util::to_bytes::<_, 512>(&p).unwrap();
    let mut s = rkyv::ser::serializers::AllocSerializer::<512>::default();
    s.serialize_unsized_value(&p).unwrap();
    let n = s.pos();
    dbg!(n);
    let (s, _, _) = s.into_components();

    let mut buff = Vec::with_capacity(1024);
    unsafe { buff.set_len(1024) };
    buff.fill(0);

    let encode_n = mais::encode(&s.into_inner()[0..n], &mut buff).unwrap();

    // --------------
    let buf = b"lmap_rangepages=PageRange<2Mb>(0x1000000000..0x1100000000),frames=FrameRange<2Mb>(0x0..0x100000000),flags=3,\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x02\x08\x08\x01\x01\x01\x01\x01\x01\x06\x80\xff\xff\xff\t\x01\x01\x06\x81\xff\xff\xffb\x01\x01\x05\xe0\xff\xff\xff\x00\x02\x02\x01\x01\x01\x01\x01\x01\x02\x08\x08\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01";
    // assert_eq!(&buff[..encode_n], buf);

    // return;

    let buf = b"lmap_rangepages=PageRange<2Mb>(0x1000000000..0x1100000000),frames=FrameRange<2Mb>(0x0..0x100000000),flags=3,\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x02\x08\x01\x01\x01\x01\x01\x01\x06\x80\xff\xff\xff\t\x01\x01\x06\x81\xff\xff\xffb\x01\x01\x05\xe0\xff\xff\xff\x00\x02\x02\x01\x01\x01\x01\x01\x01\x02\x08\x08\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01";

    let mut bytes = BytesMut::new();
    bytes.extend_from_slice(buf);
    bytes.truncate(bytes.len() - 0);
    bytes.put_u8(0);

    let item: LogRef = LogDecoder.decode(&mut bytes).unwrap().unwrap();

    match item.as_ref() {
        ArchivedLogPacket::NewSpan(span) => {
            dbg!(&*span.fields);
        }
        _ => unreachable!(),
    }
}
