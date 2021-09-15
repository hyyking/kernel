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
        $vis struct $bitfield ($T);
        impl $bitfield {
        $(
            $crate::paste::paste! {
                $ivis fn [<get_ $bit>](&self) -> $T {
                    self.0.get_bits($idx)
                }
            }

            $crate::paste::paste! {
                $ivis fn [<set_ $bit>](&mut self, val: $T) {
                    self.0.set_bits($idx, val);
                }
            }
        )*
        }
    }

}
