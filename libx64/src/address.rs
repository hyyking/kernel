use core::ptr::NonNull;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VirtualAddr(u64);

impl core::fmt::Debug for VirtualAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("VirtualAddr")
            .field(&format_args!("{:#02x}", self.0))
            .finish()
    }
}

impl VirtualAddr {
    pub const fn new(addr: u64) -> Result<Self, u64> {
        match addr >> 47 {
            0 | 0x1FFFF => Ok(Self(addr)),
            1 => Ok(Self(((addr << 16) as i64 >> 16) as u64)),
            _ => Err(addr),
        }
    }

    pub fn ptr<T>(&self) -> Option<NonNull<T>> {
        NonNull::new(self.0 as *mut T)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}
