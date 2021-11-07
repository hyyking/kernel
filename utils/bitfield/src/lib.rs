#![no_std]

#[doc(hidden)]
pub extern crate paste;

#[macro_export]
macro_rules! bitfield {
    (
        $(#[$outer:meta])*
        $vis:vis unsafe struct $bitfield:ident: $T:ty {
            $(
                $(#[$inner:ident $($args:tt)*])*
                $ivis:vis $bit:ident: $idx:expr,
            )*
        }
    ) => {

        $(#[$outer])*
        ///
        /// ## Warning:
        ///
        /// This structure is a bitfield, overlaping bits are not checked for at runtime.
        /// Please check the implementation before settings any bits. Bit ranges are also displayed
        /// in each method's documentation.
        $vis struct $bitfield ($T);

        impl $bitfield {
            const BITS: usize = ::core::mem::size_of::<$T>() * 8;

            $vis const fn zero() -> Self {
                Self(0)
            }

            $vis const unsafe fn raw(value: $T) -> Self {
                Self(value)
            }

            $crate::paste::paste! {
                $vis const fn [<as_ $T>](self) -> $T {
                    self.0
                }
            }
        $(

            $crate::paste::paste! {
                $(#[$inner $($args)*])*
                ///
                #[doc = concat!(" ", stringify!(_Bitfield_: This field covers the exclusive range $idx))]
                $ivis const fn [<get_ $bit>](&self) -> $T {
                    const RANGE: (usize, usize) = $crate::decompose_range($idx);
                    let bits = self.0 << (Self::BITS - RANGE.1) >> (Self::BITS - RANGE.1);
                    bits >> RANGE.0
                }
            }

            $crate::paste::paste! {
                $(#[$inner $($args)*])*
                ///
                /// ## Range:
                #[doc = concat!(" ", stringify!(This field covers the range: $idx))]
                ///
                $ivis const fn [<set_ $bit>](self, val: $T) -> Self {
                    const RANGE: (usize, usize) = $crate::decompose_range($idx);
                    let bitmask = !(!0 << (Self::BITS - RANGE.1) >>
                                (Self::BITS - RANGE.1) >>
                                RANGE.0 << RANGE.0);
                    Self((self.0 & bitmask) | (val << RANGE.0))
                }
            }
        )*
        }


        impl ::core::fmt::Debug for $bitfield {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> Result<(), ::core::fmt::Error> {
                let mut s = f.debug_struct(stringify!($bitfield));
                let a = s.field(".0", &format_args!("{:#0b}", self.0));
                $(
                    $crate::paste::paste! {
                        let a = a.field(stringify!( > $bit), &{ self.[<get_ $bit>]() });
                    }
                )*
                a.finish()
            }
        }
    }
}

#[doc(hidden)]
#[must_use]
pub const fn decompose_range(range: core::ops::Range<usize>) -> (usize, usize) {
    (range.start, range.end)
}
