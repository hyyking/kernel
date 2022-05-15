#![allow(unused)]

use core::ops::{Deref, DerefMut};

#[derive(Clone)]
#[repr(transparent)]
pub struct Volatile<T> {
    value: T,
}

impl<T, R> Volatile<R>
where
    R: Deref<Target = T>,
{
    pub fn new(value: R) -> Self {
        Self { value }
    }

    pub fn read(&self) -> T {
        // SAFETY: our internal value exists.
        unsafe { core::ptr::read_volatile(&*self.value) }
    }

    pub fn write(&mut self, value: T)
    where
        R: DerefMut,
    {
        // SAFETY: our internal value exists.
        unsafe { core::ptr::write_volatile(&mut *self.value, value) };
    }

    pub fn update<F>(&mut self, f: F)
    where
        R: DerefMut,
        F: FnOnce(&mut T),
    {
        let mut value = self.read();
        f(&mut value);
        self.write(value);
    }
}
