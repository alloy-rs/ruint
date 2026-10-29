//! Signed integers.

use crate::Uint;

mod add;
mod bits;
mod bytes;
mod cmp;
mod div;
mod fmt;
mod from;
mod mul;
mod pow;
mod sign;
mod string;

pub use sign::Sign;

/// The ring of numbers modulo $2^{\mathtt{BITS}}$, interpreted as two's
/// complement signed integers in the range $[-2^{\mathtt{BITS}-1},
/// 2^{\mathtt{BITS}-1})$.
///
/// [`Int`] wraps a [`Uint`] holding the two's complement representation, and
/// implements the methods of the `std` signed integer types, following the
/// same conventions as [`Uint`].
///
/// # Notable differences from `std` int types.
///
/// * The operators `+`, `-`, `*`, `/`, etc. use wrapping math, like [`Uint`].
///   Division and remainder by zero still panic.
/// * The [`Int::checked_shl`], [`Int::overflowing_shl`], etc. return overflow
///   when the result does not equal $\mathtt{self} ⋅ 2^{\mathtt{rhs}}$, and
///   [`Int::checked_shr`], [`Int::overflowing_shr`], etc. return overflow when
///   non-zero bits are shifted out. In std they return overflow when the shift
///   amount is greater than the bit size.
/// * [`Int::pow`] and friends take a [`Uint`] exponent.
/// * [`Int::to_le_bytes`] and [`Int::to_be_bytes`] require the output size to
///   be provided as a const-generic argument. They will runtime panic if the
///   provided size is incorrect.
///
/// # Examples
///
/// ```
/// # use ruint::{aliases::I256, uint};
/// # uint! {
/// let a = -5_I256;
/// let b = "-0x10".parse::<I256>().unwrap();
/// assert_eq!(a * b, 80_I256);
/// assert_eq!(b / a, 3_I256);
/// assert_eq!(b % a, -1_I256);
/// assert!(b < a);
/// assert_eq!(I256::MIN.checked_neg(), None);
/// # }
/// ```
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Int<const BITS: usize, const LIMBS: usize>(Uint<BITS, LIMBS>);

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// The size of this integer type in 64-bit limbs.
    pub const LIMBS: usize = Uint::<BITS, LIMBS>::LIMBS;

    /// The size of this integer type in bits.
    pub const BITS: usize = BITS;

    /// The value zero.
    pub const ZERO: Self = Self(Uint::ZERO);

    /// The value one.
    ///
    /// Zero if `BITS` is less than two.
    pub const ONE: Self = if BITS < 2 {
        Self::ZERO
    } else {
        Self(Uint::ONE)
    };

    /// The value minus one.
    ///
    /// Zero if `BITS` is zero.
    pub const MINUS_ONE: Self = Self(Uint::MAX);

    /// The smallest value that can be represented by this integer type,
    /// $-2^{\mathtt{BITS}-1}$.
    ///
    /// Zero if `BITS` is zero.
    pub const MIN: Self = Self::MAX.not();

    /// The largest value that can be represented by this integer type,
    /// $2^{\mathtt{BITS}-1} − 1$.
    pub const MAX: Self = Self(Uint::MAX.wrapping_shr(1));

    /// View the array of limbs of the two's complement representation.
    #[inline(always)]
    #[must_use]
    pub const fn as_limbs(&self) -> &[u64; LIMBS] {
        self.0.as_limbs()
    }

    /// Convert to an array of limbs of the two's complement representation.
    ///
    /// Limbs are least significant first.
    #[inline(always)]
    #[must_use]
    pub const fn into_limbs(self) -> [u64; LIMBS] {
        self.0.into_limbs()
    }

    /// Construct a new integer from a little-endian array of limbs of the
    /// two's complement representation.
    ///
    /// # Panics
    ///
    /// Panics if `LIMBS` is not equal to `nlimbs(BITS)`.
    ///
    /// Panics if bits above `BITS` are set.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn from_limbs(limbs: [u64; LIMBS]) -> Self {
        Self(Uint::from_limbs(limbs))
    }

    /// Returns the two's complement representation of `self` as a [`Uint`].
    ///
    /// This is the same as `self as u64` for [`i64`].
    #[inline(always)]
    #[must_use]
    pub const fn cast_unsigned(self) -> Uint<BITS, LIMBS> {
        self.0
    }

    /// Returns the sign of `self`.
    #[inline]
    #[must_use]
    pub const fn sign(&self) -> Sign {
        if self.is_negative() {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }

    /// Returns `true` if `self` is negative.
    #[inline]
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        BITS != 0 && self.0.bit(BITS - 1)
    }

    /// Returns `true` if `self` is positive. Zero is not positive.
    #[inline]
    #[must_use]
    pub const fn is_positive(&self) -> bool {
        !self.is_negative() && !self.0.const_is_zero()
    }

    /// Returns a number representing the sign of `self`:
    ///
    /// * `0` if the number is zero,
    /// * `1` if the number is positive,
    /// * `-1` if the number is negative.
    #[inline]
    #[must_use]
    pub const fn signum(self) -> Self {
        if self.is_negative() {
            Self::MINUS_ONE
        } else if self.0.const_is_zero() {
            Self::ZERO
        } else {
            Self::ONE
        }
    }

    /// Returns the absolute value of `self` as a [`Uint`], without wrapping
    /// or panicking.
    #[inline]
    #[must_use]
    pub const fn unsigned_abs(self) -> Uint<BITS, LIMBS> {
        self.into_sign_and_abs().1
    }

    /// Splits `self` into its sign and absolute value.
    #[inline]
    #[must_use]
    pub const fn into_sign_and_abs(self) -> (Sign, Uint<BITS, LIMBS>) {
        if self.is_negative() {
            (Sign::Negative, self.0.wrapping_neg())
        } else {
            (Sign::Positive, self.0)
        }
    }

    /// Creates an [`Int`] from a sign and an absolute value, returning
    /// [`None`] if the value does not fit.
    ///
    /// Zero is accepted with either sign.
    #[inline]
    #[must_use]
    pub const fn checked_from_sign_and_abs(sign: Sign, abs: Uint<BITS, LIMBS>) -> Option<Self> {
        match Self::overflowing_from_sign_and_abs(sign, abs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Creates an [`Int`] from a sign and an absolute value.
    ///
    /// Returns the value wrapped modulo $2^{\mathtt{BITS}}$ and a boolean
    /// indicating whether it overflowed. Zero is accepted with either sign.
    #[inline]
    #[must_use]
    pub const fn overflowing_from_sign_and_abs(sign: Sign, abs: Uint<BITS, LIMBS>) -> (Self, bool) {
        match sign {
            Sign::Positive => {
                let value = Self(abs);
                (value, value.is_negative())
            }
            Sign::Negative => {
                let value = Self(abs.wrapping_neg());
                (value, !value.is_negative() && !abs.const_is_zero())
            }
        }
    }
}

impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    /// Returns the [`Int`] with the same two's complement representation as
    /// `self`.
    ///
    /// This is the same as `self as i64` for [`u64`].
    #[inline(always)]
    #[must_use]
    pub const fn cast_signed(self) -> Int<BITS, LIMBS> {
        Int(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{I0, I1, I8, I128, I256},
        const_for, nlimbs,
    };
    use proptest::proptest;

    #[test]
    fn test_constants() {
        assert_eq!(I0::MIN, I0::ZERO);
        assert_eq!(I0::MAX, I0::ZERO);
        assert_eq!(I0::ONE, I0::ZERO);
        assert_eq!(I0::MINUS_ONE, I0::ZERO);
        assert_eq!(I1::MIN, I1::MINUS_ONE);
        assert_eq!(I1::MAX, I1::ZERO);
        assert_eq!(I1::ONE, I1::ZERO);
        assert_eq!(I8::MIN.cast_unsigned(), Uint::from(0x80));
        assert_eq!(I8::MAX.cast_unsigned(), Uint::from(0x7f));
        assert_eq!(I8::MINUS_ONE.cast_unsigned(), Uint::from(0xff));
        assert_eq!(I128::MIN, I128::try_from(i128::MIN).unwrap());
        assert_eq!(I128::MAX, I128::try_from(i128::MAX).unwrap());
        assert_eq!(I256::ONE.into_limbs(), [1, 0, 0, 0]);
        assert_eq!(I256::MIN.into_limbs(), [0, 0, 0, 1 << 63]);
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            assert!(I::MIN.is_negative());
            assert!(!I::MAX.is_negative());
            assert_eq!(I::MIN.cast_unsigned(), Uint::ONE << (BITS - 1));
            assert_eq!(I::MAX.wrapping_add(I::MINUS_ONE.wrapping_neg()), I::MIN);
        });
    }

    #[test]
    fn test_sign_and_abs() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(value: I)| {
                let (sign, abs) = value.into_sign_and_abs();
                assert_eq!(I::checked_from_sign_and_abs(sign, abs), Some(value));
                assert_eq!(abs, if value.is_negative() { (-value).cast_unsigned() } else { value.cast_unsigned() });
            });
        });
        assert_eq!(
            I8::overflowing_from_sign_and_abs(Sign::Negative, Uint::from(128)),
            (I8::MIN, false)
        );
        assert_eq!(
            I8::overflowing_from_sign_and_abs(Sign::Positive, Uint::from(128)),
            (I8::MIN, true)
        );
        assert_eq!(
            I8::overflowing_from_sign_and_abs(Sign::Negative, Uint::from(129)),
            (I8::MAX, true)
        );
        assert_eq!(
            I8::overflowing_from_sign_and_abs(Sign::Negative, Uint::ZERO),
            (I8::ZERO, false)
        );
    }

    #[test]
    fn test_signum() {
        assert_eq!(I256::MIN.signum(), I256::MINUS_ONE);
        assert_eq!(I256::ZERO.signum(), I256::ZERO);
        assert_eq!(I256::MAX.signum(), I256::ONE);
        assert!(I256::MAX.is_positive());
        assert!(!I256::ZERO.is_positive());
        assert!(!I256::ZERO.is_negative());
        assert_eq!(I1::MINUS_ONE.signum(), I1::MINUS_ONE);
    }
}
