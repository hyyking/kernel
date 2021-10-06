use core::ptr::NonNull;

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct VirtualAddr(u64);

impl VirtualAddr {
    pub const fn new(addr: u64) -> Self {
        match addr >> 47 {
            0 | 0x1FFFF => Self(addr),
            1 => Self(((addr << 16) as i64 >> 16) as u64),
            _ => panic!(),
        }
    }

    pub fn ptr<T>(&self) -> Option<NonNull<T>> {
        NonNull::new(self.0 as *mut T)
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as u64)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub const fn null() -> VirtualAddr {
        Self(0)
    }
}

impl core::fmt::Debug for VirtualAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("VirtualAddr")
            .field(&format_args!("{:#02x}", self.0))
            .finish()
    }
}
