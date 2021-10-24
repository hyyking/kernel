use crate::{
    address::VirtualAddr,
    paging::{PageCheck, PageSize},
};

#[derive(Debug, Clone, Copy)]
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
