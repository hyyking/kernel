use core::cell::UnsafeCell;
use core::ops::Deref;

use crate::sync::mutex::SpinMutex;

#[macro_export]
macro_rules! klazy {
    ( $(#[$attr:meta])* $v:vis ref static $name:ident: $t:ty = $b:expr;) => {
        $(#[$attr])*
        $v static $name: $crate::sync::lazy::Lazy<$t> = $crate::sync::lazy::Lazy::new(|| $b);
    }
}

pub struct Lazy<T, F = fn() -> T> {
    f: SpinMutex<Option<F>>,
    data: UnsafeCell<Option<T>>,
}

unsafe impl<T, F: Send> Sync for Lazy<T, F> {}

impl<T, F> Lazy<T, F> {
    pub const fn new(f: F) -> Self {
        Self {
            f: SpinMutex::new(Some(f)),
            data: UnsafeCell::new(None),
        }
    }
}

impl<T, F: FnOnce() -> T> Lazy<T, F> {
    fn get_or_init(&self) -> &T {
        if let Some(f) = SpinMutex::try_poison(&self.f).and_then(Option::take) {
            unsafe {
                *self.data.get() = Some(f());
            }
        }
        unsafe { (*self.data.get()).as_ref().unwrap() }
    }
}

impl<T, F: FnOnce() -> T> Deref for Lazy<T, F> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.get_or_init()
    }
}
