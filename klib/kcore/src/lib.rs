#![no_std]
#![feature(allocator_api, slice_ptr_get, slice_ptr_len)]
#![allow(clippy::cast_possible_truncation)]

#[cfg(test)]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod either;

pub mod ptr;

pub mod queue {
    pub use crossbeam_queue::*;
}

pub mod futures {
    #[cfg(feature = "alloc")]
    pub mod task {
        pub use futures_util::task::{waker, ArcWake, AtomicWaker};
    }

    pub mod stream {
        pub use futures_util::stream::*;
    }
}

pub mod sync;
pub mod tables;

pub mod error {
    pub trait Error {}

    impl Error for &'static str {}
}
