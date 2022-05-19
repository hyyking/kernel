#![allow(clippy::module_name_repetitions)]

use rkyv::{with::RefAsBox, Archive, Serialize};

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
pub enum Level {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

pub const HEADER_SIZE: usize = core::mem::size_of::<<LogHeader as rkyv::Archive>::Archived>();

// NOTE: protocols are built with size_32 for now
// #[cfg(feature = "rkyv/size_32")]
pub const SIZE_PAD: usize = 4;

#[derive(Archive, Serialize, rkyv::Deserialize, Debug)]
pub struct LogHeader {
    pub size: usize,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
pub struct LogMessage<'a> {
    pub level: Level,
    pub line: usize,

    #[with(rkyv::with::RefAsBox)]
    pub path: &'a str,

    #[with(RefAsBox)]
    pub message: &'a str,
}
