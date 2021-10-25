use crate::{
    address::VirtualAddr,
    paging::{PageCheck, PageSize},
};

#[derive(Clone, Copy)]
pub struct Page<const N: u64>
where
    PageCheck<N>: PageSize,
{
    addr: VirtualAddr,
}

impl<const N: u64> Page<N>
where
    PageCheck<N>: PageSize,
{
    pub const fn containing(addr: VirtualAddr) -> Self {
        Self {
            addr: addr.align_down(N),
        }
    }

    pub const fn ptr(self) -> VirtualAddr {
        self.addr
    }
}

impl<const N: u64> core::fmt::Debug for Page<N>
where
    PageCheck<N>: PageSize,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page")
            .field("size", &N)
            .field("ptr", &format_args!("{:#x}", &self.addr.as_u64()))
            .finish()
    }
}
