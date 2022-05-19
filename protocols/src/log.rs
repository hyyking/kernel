use rkyv::with::RefAsBox;

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
)]
#[archive_attr(derive(Debug, Clone, Copy))]
// #[cfg_attr(test, archive_attr(derive(bytecheck::CheckBytes)))]
#[repr(u8)]
pub enum Level {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
// #[cfg_attr(test, archive_attr(derive(bytecheck::CheckBytes)))]
pub enum LogPacket<'a> {
    NewSpan(Span<'a>),
    Message(Message<'a>),
    EnterSpan(u64),
    ExitSpan(u64),
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
// #[cfg_attr(test, archive_attr(derive(bytecheck::CheckBytes)))]
pub struct Span<'a> {
    pub id: u64,

    #[with(RefAsBox)]
    pub target: &'a str,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
// #[cfg_attr(test, archive_attr(derive(bytecheck::CheckBytes)))]
pub struct Message<'a> {
    pub level: Level,
    pub line: u32,

    #[with(RefAsBox)]
    pub path: &'a str,

    #[with(RefAsBox)]
    pub message: &'a str,
}

#[cfg(test)]
mod test {
    use rkyv::ser::Serializer;

    use super::*;

    #[test]
    fn span_message() {
        let p = LogPacket::NewSpan(Span {
            id: 1,
            target: "bios",
        });
        // let a = rkyv::util::to_bytes::<_, 512>(&p).unwrap();
        let mut s = rkyv::ser::serializers::AllocSerializer::<512>::default();
        s.serialize_unsized_value(&p).unwrap();
        let (s, _, _) = s.into_components();
        let a = s.into_inner();

        let offset = &a[..a.len() - 0];
        unsafe {
            let packet = rkyv::archived_unsized_root::<LogPacket>(offset);
            match packet {
                ArchivedLogPacket::NewSpan(ref span) => {
                    std::dbg!(&*span.target);
                }
                _ => panic!(),
            }
        }

        std::dbg!(p, offset);
    }
}
