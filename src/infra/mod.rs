#![allow(unused_macros)]

macro_rules! cfg_qemu {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "qemu")]
            #[cfg_attr(docsrs, doc(cfg(feature = "qemu")))]
            $item
        )*
    }
}

macro_rules! cfg_not_qemu {
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "qemu"))]
            #[cfg_attr(docsrs, doc(cfg(not(feature = "qemu"))))]
            $item
        )*
    }
}

pub mod qemu;
#[cfg(test)]
pub mod tests;
