use crate::{Int, Uint};
use core::{
    iter::Sum,
    ops::{Add, AddAssign, Neg, Sub, SubAssign},
};

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// Computes the absolute value of `self`, wrapping around at the boundary
    /// of the type.
    ///
    /// The absolute value of [`Int::MIN`] is [`Int::MIN`].
    #[inline(always)]
    #[must_use]
    pub const fn abs(self) -> Self {
        self.wrapping_abs()
    }

    /// Computes the absolute value of `self`, returning [`None`] if `self ==
    /// MIN`.
    #[inline(always)]
    #[must_use]
    pub const fn checked_abs(self) -> Option<Self> {
        match self.overflowing_abs() {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes the absolute value of `self`, panicking if `self == MIN`.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn strict_abs(self) -> Self {
        match self.overflowing_abs() {
            (value, false) => value,
            _ => panic!("attempt to negate with overflow"),
        }
    }

    /// Computes the absolute value of `self`.
    ///
    /// Returns a tuple of the absolute value along with a boolean indicating
    /// whether an overflow happened. The absolute value of [`Int::MIN`]
    /// overflows to [`Int::MIN`].
    #[inline(always)]
    #[must_use]
    pub const fn overflowing_abs(self) -> (Self, bool) {
        if self.is_negative() {
            self.overflowing_neg()
        } else {
            (self, false)
        }
    }

    /// Computes the absolute value of `self`, returning [`Int::MAX`] if `self
    /// == MIN`.
    #[inline(always)]
    #[must_use]
    pub const fn saturating_abs(self) -> Self {
        match self.overflowing_abs() {
            (value, false) => value,
            _ => Self::MAX,
        }
    }

    /// Computes the absolute value of `self`, wrapping around at the boundary
    /// of the type.
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_abs(self) -> Self {
        self.overflowing_abs().0
    }

    /// Computes the absolute difference between `self` and `other`.
    ///
    /// Returns $\left\vert \mathtt{self} - \mathtt{other} \right\vert$ as a
    /// [`Uint`], which can not overflow.
    #[inline]
    #[must_use]
    pub fn abs_diff(self, other: Self) -> Uint<BITS, LIMBS> {
        if self < other {
            other.0.wrapping_sub(self.0)
        } else {
            self.0.wrapping_sub(other.0)
        }
    }

    /// Computes `self + rhs`, returning [`None`] if overflow occurred.
    #[inline(always)]
    #[must_use]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.overflowing_add(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes `self + rhs`, panicking if overflow occurred.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn strict_add(self, rhs: Self) -> Self {
        match self.overflowing_add(rhs) {
            (value, false) => value,
            _ => panic!("attempt to add with overflow"),
        }
    }

    /// Computes `-self`, returning [`None`] if `self == MIN`.
    #[inline(always)]
    #[must_use]
    pub const fn checked_neg(self) -> Option<Self> {
        match self.overflowing_neg() {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes `-self`, panicking if `self == MIN`.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn strict_neg(self) -> Self {
        match self.overflowing_neg() {
            (value, false) => value,
            _ => panic!("attempt to negate with overflow"),
        }
    }

    /// Computes `self - rhs`, returning [`None`] if overflow occurred.
    #[inline(always)]
    #[must_use]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.overflowing_sub(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes `self - rhs`, panicking if overflow occurred.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn strict_sub(self, rhs: Self) -> Self {
        match self.overflowing_sub(rhs) {
            (value, false) => value,
            _ => panic!("attempt to subtract with overflow"),
        }
    }

    /// Calculates `self + rhs`.
    ///
    /// Returns a tuple of the addition along with a boolean indicating whether
    /// an arithmetic overflow would occur. If an overflow would have occurred
    /// then the wrapped value is returned.
    #[inline]
    #[must_use]
    pub const fn overflowing_add(self, rhs: Self) -> (Self, bool) {
        let result = Self(self.0.wrapping_add(rhs.0));
        let negative = self.is_negative();
        let overflow = negative == rhs.is_negative() && negative != result.is_negative();
        (result, overflow)
    }

    /// Calculates `-self`.
    ///
    /// Returns a tuple of the negation along with a boolean indicating whether
    /// an overflow happened. Only [`Int::MIN`] overflows, to itself.
    #[inline]
    #[must_use]
    pub const fn overflowing_neg(self) -> (Self, bool) {
        let result = Self(self.0.wrapping_neg());
        (result, self.is_negative() && result.is_negative())
    }

    /// Calculates `self - rhs`.
    ///
    /// Returns a tuple of the subtraction along with a boolean indicating
    /// whether an arithmetic overflow would occur. If an overflow would have
    /// occurred then the wrapped value is returned.
    #[inline]
    #[must_use]
    pub const fn overflowing_sub(self, rhs: Self) -> (Self, bool) {
        let result = Self(self.0.wrapping_sub(rhs.0));
        let negative = self.is_negative();
        let overflow = negative != rhs.is_negative() && negative != result.is_negative();
        (result, overflow)
    }

    /// Computes `self + rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    #[inline(always)]
    #[must_use]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        match self.overflowing_add(rhs) {
            (value, false) => value,
            _ => self.saturated(),
        }
    }

    /// Computes `-self`, returning [`Int::MAX`] if `self == MIN`.
    #[inline(always)]
    #[must_use]
    pub const fn saturating_neg(self) -> Self {
        match self.overflowing_neg() {
            (value, false) => value,
            _ => Self::MAX,
        }
    }

    /// Computes `self - rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    #[inline(always)]
    #[must_use]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        match self.overflowing_sub(rhs) {
            (value, false) => value,
            _ => self.saturated(),
        }
    }

    /// Computes `self + rhs`, wrapping around at the boundary of the type.
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_add(self, rhs: Self) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }

    /// Computes `-self`, wrapping around at the boundary of the type.
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_neg(self) -> Self {
        Self(self.0.wrapping_neg())
    }

    /// Computes `self - rhs`, wrapping around at the boundary of the type.
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_sub(self, rhs: Self) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }

    /// The bound that an addition or subtraction overflowing from `self`
    /// saturates to.
    #[inline(always)]
    const fn saturated(self) -> Self {
        if self.is_negative() {
            Self::MIN
        } else {
            Self::MAX
        }
    }
}

impl<const BITS: usize, const LIMBS: usize> Neg for Int<BITS, LIMBS> {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        self.wrapping_neg()
    }
}

impl<const BITS: usize, const LIMBS: usize> Neg for &Int<BITS, LIMBS> {
    type Output = Int<BITS, LIMBS>;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        self.wrapping_neg()
    }
}

impl<const BITS: usize, const LIMBS: usize> Sum<Self> for Int<BITS, LIMBS> {
    #[inline]
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Self::ZERO, Self::wrapping_add)
    }
}

impl<'a, const BITS: usize, const LIMBS: usize> Sum<&'a Self> for Int<BITS, LIMBS> {
    #[inline]
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = &'a Self>,
    {
        iter.copied().fold(Self::ZERO, Self::wrapping_add)
    }
}

impl_bin_op!(Int, Add, add, AddAssign, add_assign, wrapping_add);
impl_bin_op!(Int, Sub, sub, SubAssign, sub_assign, wrapping_sub);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{I8, I64, I128, I256},
        const_for, nlimbs,
    };
    use proptest::proptest;

    #[test]
    fn test_i64_reference() {
        proptest!(|(a: i64, b: i64)| {
            let (x, y) = (I64::try_from(a).unwrap(), I64::try_from(b).unwrap());
            let i = |v: i64| I64::try_from(v).unwrap();
            assert_eq!(x.overflowing_add(y), (i(a.overflowing_add(b).0), a.overflowing_add(b).1));
            assert_eq!(x.overflowing_sub(y), (i(a.overflowing_sub(b).0), a.overflowing_sub(b).1));
            assert_eq!(x.overflowing_neg(), (i(a.overflowing_neg().0), a.overflowing_neg().1));
            assert_eq!(x.overflowing_abs(), (i(a.overflowing_abs().0), a.overflowing_abs().1));
            assert_eq!(x.saturating_add(y), i(a.saturating_add(b)));
            assert_eq!(x.saturating_sub(y), i(a.saturating_sub(b)));
            assert_eq!(x.saturating_neg(), i(a.saturating_neg()));
            assert_eq!(x.saturating_abs(), i(a.saturating_abs()));
            assert_eq!(x.abs_diff(y), Uint::from(a.abs_diff(b)));
        });
    }

    #[test]
    fn test_small_exhaustive() {
        for a in i8::MIN..=i8::MAX {
            let x = I8::try_from(a).unwrap();
            let i = |v: i8| I8::try_from(v).unwrap();
            assert_eq!(x.checked_neg(), a.checked_neg().map(i));
            assert_eq!(x.checked_abs(), a.checked_abs().map(i));
            assert_eq!(x.unsigned_abs(), Uint::from(a.unsigned_abs()));
            for b in i8::MIN..=i8::MAX {
                let y = I8::try_from(b).unwrap();
                assert_eq!(x.checked_add(y), a.checked_add(b).map(i));
                assert_eq!(x.checked_sub(y), a.checked_sub(b).map(i));
                assert_eq!(x.saturating_add(y), i(a.saturating_add(b)));
                assert_eq!(x.saturating_sub(y), i(a.saturating_sub(b)));
                assert_eq!(x + y, i(a.wrapping_add(b)));
                assert_eq!(x - y, i(a.wrapping_sub(b)));
            }
        }
    }

    #[test]
    fn test_inverse() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(a: I, b: I)| {
                assert_eq!(a + b, b + a);
                assert_eq!(a + b - b, a);
                assert_eq!(a + (-a), I::ZERO);
                assert_eq!(-(-a), a);
                assert_eq!(a.abs_diff(b), b.abs_diff(a));
            });
        });
    }

    #[test]
    fn test_i256_bounds() {
        assert_eq!(I256::MAX.overflowing_add(I256::ONE), (I256::MIN, true));
        assert_eq!(I256::MIN.overflowing_sub(I256::ONE), (I256::MAX, true));
        assert_eq!(I256::MIN.overflowing_neg(), (I256::MIN, true));
        assert_eq!(I256::MIN.abs(), I256::MIN);
        assert_eq!(I256::MIN.saturating_abs(), I256::MAX);
        assert_eq!(I256::MIN.unsigned_abs(), Uint::ONE << 255);
        assert_eq!(I256::MIN.abs_diff(I256::MAX), Uint::MAX);
        assert_eq!(I256::MAX.saturating_add(I256::MAX), I256::MAX);
        assert_eq!(I256::MIN.saturating_add(I256::MIN), I256::MIN);
        assert_eq!(I256::MIN.saturating_sub(I256::ONE), I256::MIN);
        assert_eq!(I256::MAX.saturating_sub(I256::MINUS_ONE), I256::MAX);
        assert_eq!(
            [I256::MINUS_ONE, I256::MAX, I256::ONE].iter().sum::<I256>(),
            I256::MAX
        );
        let _ = I128::ZERO.strict_add(I128::ONE);
    }

    #[test]
    #[should_panic(expected = "attempt to negate with overflow")]
    fn test_strict_neg_overflow() {
        let _ = I256::MIN.strict_neg();
    }

    #[test]
    #[should_panic(expected = "attempt to add with overflow")]
    fn test_strict_add_overflow() {
        let _ = I256::MAX.strict_add(I256::ONE);
    }
}
