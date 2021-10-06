#![no_std]

extern crate bit_field;
#[doc(hidden)]
pub extern crate paste;

pub use bit_field::BitField;

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
            $vis const fn zero() -> Self {
                Self(0)
            }

            $vis const unsafe fn raw(value: $T) -> Self {
                Self(value)
            }

            $crate::paste::paste! {
                $vis fn [<as_ $T>](self) -> $T {
                    self.0
                }
            }
        $(

            $crate::paste::paste! {
                $(#[$inner $($args)*])*
                ///
                #[doc = concat!(" ", stringify!(_Bitfield_: This field covers the exclusive range $idx))]
                $ivis fn [<get_ $bit>](&self) -> $T {
                    self.0.get_bits($idx)
                }
            }

            $crate::paste::paste! {
                $(#[$inner $($args)*])*
                ///
                /// ## Range:
                #[doc = concat!(" ", stringify!(This field covers the range: $idx))]
                ///
                $ivis fn [<set_ $bit>](&mut self, val: $T) -> &mut Self {
                    self.0.set_bits($idx, val);
                    self
                }
            }
        )*
        }


        impl ::core::fmt::Debug for $bitfield {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> Result<(), ::core::fmt::Error> {
                let mut s = f.debug_struct(stringify!($bitfield));
                let a = s.field("bin", &format_args!("{:#0b}", self.0));
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
