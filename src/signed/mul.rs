use crate::{Int, Sign, Uint};
use core::{
    iter::Product,
    ops::{Mul, MulAssign},
};

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// Computes `self * rhs`, returning [`None`] if overflow occurred.
    #[inline(always)]
    #[must_use]
    pub const fn checked_mul(self, rhs: Self) -> Option<Self> {
        match self.overflowing_mul(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes `self * rhs`, panicking if overflow occurred.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn strict_mul(self, rhs: Self) -> Self {
        match self.overflowing_mul(rhs) {
            (value, false) => value,
            _ => panic!("attempt to multiply with overflow"),
        }
    }

    /// Calculates the multiplication of `self` and `rhs`.
    ///
    /// Returns a tuple of the multiplication along with a boolean indicating
    /// whether an arithmetic overflow would occur. If an overflow would have
    /// occurred then the wrapped value is returned.
    #[inline]
    #[must_use]
    pub const fn overflowing_mul(self, rhs: Self) -> (Self, bool) {
        let (abs, overflow) = self.unsigned_abs().overflowing_mul(rhs.unsigned_abs());
        let sign = if self.is_negative() == rhs.is_negative() {
            Sign::Positive
        } else {
            Sign::Negative
        };
        // Negating the wrapped absolute value gives the wrapped product.
        let (value, sign_overflow) = Self::overflowing_from_sign_and_abs(sign, abs);
        (value, overflow || sign_overflow)
    }

    /// Computes `self * rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    #[inline(always)]
    #[must_use]
    pub const fn saturating_mul(self, rhs: Self) -> Self {
        match self.overflowing_mul(rhs) {
            (value, false) => value,
            _ if self.is_negative() == rhs.is_negative() => Self::MAX,
            _ => Self::MIN,
        }
    }

    /// Computes `self * rhs`, wrapping around at the boundary of the type.
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_mul(self, rhs: Self) -> Self {
        Self(self.0.wrapping_mul(rhs.0))
    }
}

impl<const BITS: usize, const LIMBS: usize> Product<Self> for Int<BITS, LIMBS> {
    #[inline]
    fn product<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Self(Uint::ONE), Self::wrapping_mul)
    }
}

impl<'a, const BITS: usize, const LIMBS: usize> Product<&'a Self> for Int<BITS, LIMBS> {
    #[inline]
    fn product<I>(iter: I) -> Self
    where
        I: Iterator<Item = &'a Self>,
    {
        iter.copied().fold(Self(Uint::ONE), Self::wrapping_mul)
    }
}

impl_bin_op!(Int, Mul, mul, MulAssign, mul_assign, wrapping_mul);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{I1, I8, I128, I256},
        const_for, nlimbs,
    };
    use proptest::proptest;

    #[test]
    fn test_i128_reference() {
        proptest!(|(a: i128, b: i128)| {
            let i = |v: i128| I128::try_from(v).unwrap();
            let (x, y) = (i(a), i(b));
            let (value, overflow) = a.overflowing_mul(b);
            assert_eq!(x.overflowing_mul(y), (i(value), overflow));
            assert_eq!(x.saturating_mul(y), i(a.saturating_mul(b)));
            assert_eq!(x * y, i(value));
        });
    }

    #[test]
    fn test_small_exhaustive() {
        for a in i8::MIN..=i8::MAX {
            for b in i8::MIN..=i8::MAX {
                let i = |v: i8| I8::try_from(v).unwrap();
                let (x, y) = (i(a), i(b));
                let (value, overflow) = a.overflowing_mul(b);
                assert_eq!(x.overflowing_mul(y), (i(value), overflow));
                assert_eq!(x.saturating_mul(y), i(a.saturating_mul(b)));
            }
        }
    }

    #[test]
    fn test_commutative() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(a: I, b: I)| {
                assert_eq!(a.overflowing_mul(b), b.overflowing_mul(a));
                assert_eq!(a * b, a.overflowing_mul(b).0);
                assert_eq!(a * I::MINUS_ONE, -a);
            });
        });
    }

    #[test]
    fn test_i256() {
        assert_eq!(
            I256::MIN.overflowing_mul(I256::MINUS_ONE),
            (I256::MIN, true)
        );
        assert_eq!(I256::MIN.overflowing_mul(I256::ONE), (I256::MIN, false));
        assert_eq!(I256::MIN.saturating_mul(I256::MINUS_ONE), I256::MAX);
        assert_eq!(I256::MAX.saturating_mul(I256::MINUS_ONE), -I256::MAX);
        assert_eq!(
            I256::MAX.saturating_mul(I256::MAX.wrapping_neg()),
            I256::MIN
        );
        assert_eq!(
            [I256::MINUS_ONE, I256::MAX].iter().product::<I256>(),
            -I256::MAX
        );
        assert_eq!([I1::MINUS_ONE].into_iter().product::<I1>(), I1::MINUS_ONE);
    }
}
