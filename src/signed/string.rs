use crate::{BaseConvertError, Int, ParseError, Sign, Uint};
use core::str::FromStr;

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// Parse a string into an [`Int`].
    ///
    /// The string may start with a `+` or `-` sign, followed by digits in the
    /// format of [`Uint::from_str_radix`].
    ///
    /// # Errors
    ///
    /// * [`ParseError::InvalidDigit`] if the string contains a non-digit.
    /// * [`ParseError::InvalidRadix`] if the radix is larger than 64.
    /// * [`ParseError::BaseConvertError`] if the value does not fit.
    #[inline]
    pub const fn from_str_radix(src: &str, radix: u64) -> Result<Self, ParseError> {
        let (sign, src) = match split_sign(src) {
            Ok(split) => split,
            Err(e) => return Err(e),
        };
        match Uint::from_str_radix(src, radix) {
            Ok(abs) => Self::from_parsed(sign, abs),
            Err(e) => Err(e),
        }
    }

    #[inline]
    const fn from_parsed(sign: Sign, abs: Uint<BITS, LIMBS>) -> Result<Self, ParseError> {
        match Self::checked_from_sign_and_abs(sign, abs) {
            Some(value) => Ok(value),
            None => Err(ParseError::BaseConvertError(BaseConvertError::Overflow)),
        }
    }
}

impl<const BITS: usize, const LIMBS: usize> FromStr for Int<BITS, LIMBS> {
    type Err = ParseError;

    /// Parse a string into an [`Int`].
    ///
    /// The string may start with a `+` or `-` sign, followed by digits in the
    /// format of [`Uint::from_str`], including its `0x`, `0o` and `0b`
    /// prefixes.
    #[inline]
    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let (sign, src) = split_sign(src)?;
        Self::from_parsed(sign, src.parse()?)
    }
}

/// Splits a leading `+` or `-` off `src`.
///
/// Returns an error if nothing follows the sign.
#[inline]
const fn split_sign(src: &str) -> Result<(Sign, &str), ParseError> {
    let sign = match src.as_bytes() {
        [b @ (b'-' | b'+')] => return Err(ParseError::InvalidDigit(*b as char)),
        [b'-', ..] => Sign::Negative,
        [b'+', ..] => Sign::Positive,
        _ => return Ok((Sign::Positive, src)),
    };
    Ok((sign, src.split_at(1).1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aliases::{I0, I1, I8, I128, I256};
    use proptest::proptest;

    #[allow(unused_imports)]
    use alloc::{format, string::ToString};

    #[test]
    fn test_small_exhaustive() {
        for a in i16::from(i8::MIN) - 2..=i16::from(i8::MAX) + 2 {
            let expected = i8::try_from(a).ok().map(|a| I8::try_from(a).unwrap());
            assert_eq!(a.to_string().parse::<I8>().ok(), expected);
            assert_eq!(I8::from_str_radix(&format!("{a:+}"), 10).ok(), expected);
            let hex = format!("{}0x{:x}", if a < 0 { "-" } else { "" }, a.unsigned_abs());
            assert_eq!(hex.parse::<I8>().ok(), expected);
        }
    }

    #[test]
    fn test_i128_reference() {
        proptest!(|(a: i128, radix in 2u64..=36)| {
            let s = format!("{a}");
            assert_eq!(s.parse::<I128>().unwrap(), I128::try_from(a).unwrap());
            let (sign, abs) = I128::try_from(a).unwrap().into_sign_and_abs();
            let digits = abs.to_base_be(radix).map(|d| char::from_digit(d as u32, 36).unwrap()).collect::<alloc::string::String>();
            let s = format!("{}{digits}", if sign == Sign::Negative { "-" } else { "" });
            assert_eq!(I128::from_str_radix(&s, radix).unwrap(), I128::try_from(a).unwrap());
        });
    }

    #[test]
    fn test_edge_cases() {
        let overflow = ParseError::BaseConvertError(BaseConvertError::Overflow);
        assert_eq!("0".parse::<I0>(), Ok(I0::ZERO));
        assert_eq!("-0".parse::<I0>(), Ok(I0::ZERO));
        assert_eq!("1".parse::<I0>(), Err(overflow));
        assert_eq!("-1".parse::<I1>(), Ok(I1::MINUS_ONE));
        assert_eq!("1".parse::<I1>(), Err(overflow));
        assert_eq!(
            "-0x8000000000000000000000000000000000000000000000000000000000000000".parse::<I256>(),
            Ok(I256::MIN)
        );
        assert_eq!(
            "0x8000000000000000000000000000000000000000000000000000000000000000".parse::<I256>(),
            Err(overflow)
        );
        assert_eq!(
            "-0x8000000000000000000000000000000000000000000000000000000000000001".parse::<I256>(),
            Err(overflow)
        );
        assert_eq!("+1_000".parse::<I256>(), Ok(I256::try_from(1000).unwrap()));
        assert_eq!("-".parse::<I256>(), Err(ParseError::InvalidDigit('-')));
        assert_eq!(
            I256::from_str_radix("+", 10),
            Err(ParseError::InvalidDigit('+'))
        );
        assert_eq!("--1".parse::<I256>(), Err(ParseError::InvalidDigit('-')));
        assert_eq!("-+1".parse::<I256>(), Err(ParseError::InvalidDigit('+')));
        assert_eq!(
            I256::from_str_radix("-zz", 36),
            Ok(I256::try_from(-1295).unwrap())
        );
        const {
            assert!(
                matches!(I256::from_str_radix("-10", 10), Ok(v) if v.const_eq(&I256::MINUS_ONE.wrapping_mul(I256::from_limbs([10, 0, 0, 0]))))
            )
        };
    }
}
