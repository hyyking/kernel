#![allow(clippy::module_name_repetitions)]

mod lazy;
mod mutex;

pub use lazy::Lazy;
pub use mutex::{SpinMutex, SpinMutexGuard};
