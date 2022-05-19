#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "log")]
pub mod log;

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum Noop {
    Noop = 0,
}
