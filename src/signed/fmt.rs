#![allow(clippy::missing_inline_in_public_items)] // allow format functions

use crate::{Int, fmt::StackString};
use core::fmt::{self, Write};

impl<const BITS: usize, const LIMBS: usize> fmt::Debug for Int<BITS, LIMBS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<const BITS: usize, const LIMBS: usize> fmt::Display for Int<BITS, LIMBS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(small) = i128::try_from(self) {
            return fmt::Display::fmt(&small, f);
        }
        // The absolute value has at most `BITS` decimal digits.
        let mut s = StackString::<BITS>::new();
        write!(s, "{}", self.unsigned_abs())?;
        f.pad_integral(!self.is_negative(), "", s.as_str())
    }
}

/// Non-decimal bases format the two's complement representation, like the
/// primitive signed integers.
macro_rules! impl_fmt_raw {
    ($($tr:path),*) => {$(
        impl<const BITS: usize, const LIMBS: usize> $tr for Int<BITS, LIMBS> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                <_ as $tr>::fmt(&self.0, f)
            }
        }
    )*};
}

impl_fmt_raw!(fmt::Binary, fmt::Octal, fmt::LowerHex, fmt::UpperHex);

#[cfg(test)]
mod tests {
    use crate::{
        Int,
        aliases::{I8, I128, I256},
        const_for, nlimbs,
    };
    use proptest::proptest;

    #[allow(unused_imports)]
    use alloc::{format, string::ToString};

    #[test]
    fn test_small_exhaustive() {
        for a in i8::MIN..=i8::MAX {
            let x = I8::try_from(a).unwrap();
            assert_eq!(format!("{x}"), format!("{a}"));
            assert_eq!(format!("{x:+05}"), format!("{a:+05}"));
            assert_eq!(format!("{x:<6}|"), format!("{a:<6}|"));
            assert_eq!(format!("{x:?}"), format!("{a:?}"));
            assert_eq!(format!("{x:#x}"), format!("{a:#x}"));
            assert_eq!(format!("{x:X}"), format!("{a:X}"));
            assert_eq!(format!("{x:o}"), format!("{a:o}"));
            assert_eq!(format!("{x:#010b}"), format!("{a:#010b}"));
        }
    }

    #[test]
    fn test_i128_reference() {
        proptest!(|(a: i128)| {
            let x = I128::try_from(a).unwrap();
            assert_eq!(format!("{x:+050}"), format!("{a:+050}"));
            assert_eq!(format!("{x:x}"), format!("{a:x}"));
        });
    }

    #[test]
    fn test_roundtrip() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(value: I)| {
                assert_eq!(value.to_string().parse::<I>().unwrap(), value);
                let (sign, abs) = (if value.is_negative() { "-" } else { "" }, value.unsigned_abs());
                assert_eq!(format!("{value:>1000}"), format!("{:>1000}", format!("{sign}{abs}")));
            });
        });
    }

    #[test]
    fn test_i256() {
        assert_eq!(
            I256::MIN.to_string(),
            "-57896044618658097711785492504343953926634992332820282019728792003956564819968"
        );
        assert_eq!(
            format!("{:+}", I256::MAX),
            "+57896044618658097711785492504343953926634992332820282019728792003956564819967"
        );
        assert_eq!(format!("{:x}", I256::MINUS_ONE), "f".repeat(64));
        assert_eq!(
            format!("{:#x}", I256::MIN),
            format!("0x8{}", "0".repeat(63))
        );
    }
}
