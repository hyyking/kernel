use core::marker::PhantomData;

pub struct PortReadWrite;
pub struct PortRead;
pub struct PortWrite;

pub trait PortAccess {}
impl PortAccess for PortReadWrite {}
impl PortAccess for PortRead {}
impl PortAccess for PortWrite {}

pub trait PortValue {
    /// # Safety
    ///
    /// You shall not read from an invalid port
    unsafe fn read(port: u16) -> Self;

    /// # Safety
    ///
    /// You shall not write to an invalid port
    unsafe fn write(port: u16, value: Self)
    where
        Self: Sized;
}

impl PortValue for u8 {
    #[inline]
    unsafe fn read(port: u16) -> Self
    where
        Self: Sized,
    {
        let value: u8;
        asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
        value
    }

    #[inline]
    unsafe fn write(port: u16, value: Self)
    where
        Self: Sized,
    {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

impl PortValue for u16 {
    #[inline]
    unsafe fn read(port: u16) -> Self
    where
        Self: Sized,
    {
        let value: u16;
        asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack, preserves_flags));
        value
    }

    #[inline]
    unsafe fn write(port: u16, value: Self)
    where
        Self: Sized,
    {
        asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
    }
}

impl PortValue for u32 {
    #[inline]
    unsafe fn read(port: u16) -> Self
    where
        Self: Sized,
    {
        let value: u32;
        asm!("in eax, dx", out("eax") value, in("dx") port, options(nomem, nostack, preserves_flags));
        value
    }

    #[inline]
    unsafe fn write(port: u16, value: Self)
    where
        Self: Sized,
    {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags));
    }
}

pub struct Port<V, A> {
    base: u16,
    _p: PhantomData<(V, A)>,
}

pub type RWPort<T> = Port<T, PortReadWrite>;
pub type RPort<T> = Port<T, PortRead>;
pub type WPort<T> = Port<T, PortWrite>;

impl<V, A> Port<V, A> {
    pub const fn new(base: u16) -> Self {
        Port {
            base,
            _p: PhantomData,
        }
    }
}

impl<V: PortValue> Port<V, PortReadWrite> {
    /// # Safety
    ///
    /// You shall not read from an invalid port
    #[inline]
    pub unsafe fn read(&self) -> V {
        V::read(self.base)
    }

    /// # Safety
    ///
    /// You shall not write to an invalid port
    #[inline]
    pub unsafe fn write(&mut self, value: V) {
        V::write(self.base, value)
    }
}

impl<V: PortValue> Port<V, PortRead> {
    /// # Safety
    ///
    /// You shall not read from an invalid port
    #[inline]
    pub unsafe fn read(&self) -> V {
        V::read(self.base)
    }
}

impl<V: PortValue> Port<V, PortWrite> {
    /// # Safety
    ///
    /// You shall not write to an invalid port
    #[inline]
    pub unsafe fn write(&mut self, value: V) {
        V::write(self.base, value);
    }
}
