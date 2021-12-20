#![no_std]
#![feature(allocator_api, slice_ptr_get, slice_ptr_len)]
#![allow(clippy::cast_possible_truncation)]

#[cfg(test)]
extern crate std;

extern crate alloc;

#[macro_use]
extern crate log;

pub mod either;
pub mod kalloc;
pub mod ptr;

pub mod queue {
    pub use crossbeam_queue::*;
}

pub mod futures {
    pub mod task {
        pub use futures_util::task::{waker, ArcWake, AtomicWaker};
    }

    pub mod stream {
        pub use futures_util::stream::*;
    }
}

pub mod resource;
pub mod sync;
pub mod tables;
