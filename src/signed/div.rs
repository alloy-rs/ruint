use crate::Int;
use core::ops::{Div, DivAssign, Rem, RemAssign};

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// Computes `self / rhs`, returning [`None`] if `rhs == 0` or the division
    /// overflows.
    #[inline]
    #[must_use]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.is_zero() {
            return None;
        }
        match self.overflowing_div(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes `self / rhs`.
    ///
    /// # Panics
    ///
    /// This function will panic if `rhs == 0` or the division overflows.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn strict_div(self, rhs: Self) -> Self {
        match self.overflowing_div(rhs) {
            (value, false) => value,
            _ => panic!("attempt to divide with overflow"),
        }
    }

    /// Computes `self / rhs`, rounding towards zero.
    ///
    /// Returns a tuple of the quotient along with a boolean indicating whether
    /// an overflow happened. Only `MIN / -1` overflows, to [`Int::MIN`].
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn overflowing_div(self, rhs: Self) -> (Self, bool) {
        (self.wrapping_div(rhs), self.overflows_div(rhs))
    }

    /// Computes `self / rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn saturating_div(self, rhs: Self) -> Self {
        match self.overflowing_div(rhs) {
            (value, false) => value,
            _ => Self::MAX,
        }
    }

    /// Computes `self / rhs`, rounding towards zero and wrapping around at the
    /// boundary of the type.
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn wrapping_div(self, rhs: Self) -> Self {
        self.div_rem(rhs).0
    }

    /// Computes `self % rhs`, returning [`None`] if `rhs == 0` or the division
    /// overflows.
    #[inline]
    #[must_use]
    pub fn checked_rem(self, rhs: Self) -> Option<Self> {
        if rhs.is_zero() {
            return None;
        }
        match self.overflowing_rem(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes `self % rhs`.
    ///
    /// # Panics
    ///
    /// This function will panic if `rhs == 0` or the division overflows.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn strict_rem(self, rhs: Self) -> Self {
        match self.overflowing_rem(rhs) {
            (value, false) => value,
            _ => panic!("attempt to calculate the remainder with overflow"),
        }
    }

    /// Computes `self % rhs`, with the sign of `self`.
    ///
    /// Returns a tuple of the remainder along with a boolean indicating whether
    /// the division overflowed. Only `MIN % -1` overflows, to zero.
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn overflowing_rem(self, rhs: Self) -> (Self, bool) {
        (self.wrapping_rem(rhs), self.overflows_div(rhs))
    }

    /// Computes `self % rhs`, with the sign of `self`, wrapping around at the
    /// boundary of the type.
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn wrapping_rem(self, rhs: Self) -> Self {
        self.div_rem(rhs).1
    }

    /// Computes `self / rhs` rounding towards zero, and `self % rhs` with the
    /// sign of `self`.
    ///
    /// `MIN / -1` wraps around to [`Int::MIN`].
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn div_rem(self, rhs: Self) -> (Self, Self) {
        let (q, r) = self.unsigned_abs().div_rem(rhs.unsigned_abs());
        let q = if self.is_negative() == rhs.is_negative() {
            q
        } else {
            q.wrapping_neg()
        };
        let r = if self.is_negative() {
            r.wrapping_neg()
        } else {
            r
        };
        (Self(q), Self(r))
    }

    /// Computes the quotient of the Euclidean division of `self` by `rhs`.
    ///
    /// This computes the integer `q` such that `self = q * rhs + r`, with `r =
    /// self.rem_euclid(rhs)` and `0 <= r < abs(rhs)`. `MIN.div_euclid(-1)`
    /// wraps around to [`Int::MIN`].
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn div_euclid(self, rhs: Self) -> Self {
        self.wrapping_div_euclid(rhs)
    }

    /// Computes `self.div_euclid(rhs)`, returning [`None`] if `rhs == 0` or the
    /// division overflows.
    #[inline]
    #[must_use]
    pub fn checked_div_euclid(self, rhs: Self) -> Option<Self> {
        if rhs.is_zero() {
            return None;
        }
        match self.overflowing_div_euclid(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes `self.div_euclid(rhs)`.
    ///
    /// Returns a tuple of the quotient along with a boolean indicating whether
    /// an overflow happened. Only `MIN / -1` overflows, to [`Int::MIN`].
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn overflowing_div_euclid(self, rhs: Self) -> (Self, bool) {
        (self.wrapping_div_euclid(rhs), self.overflows_div(rhs))
    }

    /// Computes `self.div_euclid(rhs)`, wrapping around at the boundary of the
    /// type.
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn wrapping_div_euclid(self, rhs: Self) -> Self {
        let (q, r) = self.div_rem(rhs);
        if !r.is_negative() {
            q
        } else if rhs.is_negative() {
            q.wrapping_add(Self::ONE)
        } else {
            q.wrapping_sub(Self::ONE)
        }
    }

    /// Computes the least non-negative remainder of `self` modulo `rhs`.
    ///
    /// This is done as if by the Euclidean division algorithm: given `r =
    /// self.rem_euclid(rhs)`, `self = rhs * self.div_euclid(rhs) + r` and `0
    /// <= r < abs(rhs)`.
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn rem_euclid(self, rhs: Self) -> Self {
        self.wrapping_rem_euclid(rhs)
    }

    /// Computes `self.rem_euclid(rhs)`, returning [`None`] if `rhs == 0` or
    /// the division overflows.
    #[inline]
    #[must_use]
    pub fn checked_rem_euclid(self, rhs: Self) -> Option<Self> {
        if rhs.is_zero() {
            return None;
        }
        match self.overflowing_rem_euclid(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Computes `self.rem_euclid(rhs)`.
    ///
    /// Returns a tuple of the remainder along with a boolean indicating whether
    /// the division overflowed. Only `MIN % -1` overflows, to zero.
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn overflowing_rem_euclid(self, rhs: Self) -> (Self, bool) {
        (self.wrapping_rem_euclid(rhs), self.overflows_div(rhs))
    }

    /// Computes `self.rem_euclid(rhs)`, wrapping around at the boundary of the
    /// type.
    ///
    /// # Panics
    ///
    /// Panics if `rhs == 0`.
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn wrapping_rem_euclid(self, rhs: Self) -> Self {
        let r = self.wrapping_rem(rhs);
        if r.is_negative() {
            // `r + abs(rhs)`, which can not overflow since `abs(r) < abs(rhs)`.
            Self(r.0.wrapping_add(rhs.unsigned_abs()))
        } else {
            r
        }
    }

    /// Returns `true` if `self / rhs` overflows, which only happens for `MIN /
    /// -1`.
    #[inline(always)]
    fn overflows_div(self, rhs: Self) -> bool {
        self == Self::MIN && rhs == Self::MINUS_ONE
    }
}

impl_bin_op!(Int, Div, div, DivAssign, div_assign, wrapping_div);
impl_bin_op!(Int, Rem, rem, RemAssign, rem_assign, wrapping_rem);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{I8, I128, I256},
        const_for, nlimbs,
    };
    use proptest::{prop_assume, proptest};

    #[test]
    fn test_i128_reference() {
        proptest!(|(a: i128, b: i128)| {
            prop_assume!(b != 0);
            let i = |v: i128| I128::try_from(v).unwrap();
            let (x, y) = (i(a), i(b));
            assert_eq!(x.overflowing_div(y), (i(a.overflowing_div(b).0), a.overflowing_div(b).1));
            assert_eq!(x.overflowing_rem(y), (i(a.overflowing_rem(b).0), a.overflowing_rem(b).1));
            assert_eq!(x.checked_div_euclid(y), a.checked_div_euclid(b).map(i));
            assert_eq!(x.checked_rem_euclid(y), a.checked_rem_euclid(b).map(i));
            assert_eq!(x.wrapping_div_euclid(y), i(a.wrapping_div_euclid(b)));
            assert_eq!(x.wrapping_rem_euclid(y), i(a.wrapping_rem_euclid(b)));
            assert_eq!(x.saturating_div(y), i(a.saturating_div(b)));
        });
    }

    #[test]
    fn test_small_exhaustive() {
        for a in i8::MIN..=i8::MAX {
            for b in i8::MIN..=i8::MAX {
                let i = |v: i8| I8::try_from(v).unwrap();
                let (x, y) = (i(a), i(b));
                assert_eq!(x.checked_div(y), a.checked_div(b).map(i));
                assert_eq!(x.checked_rem(y), a.checked_rem(b).map(i));
                assert_eq!(x.checked_div_euclid(y), a.checked_div_euclid(b).map(i));
                assert_eq!(x.checked_rem_euclid(y), a.checked_rem_euclid(b).map(i));
                if b != 0 {
                    assert_eq!(x / y, i(a.wrapping_div(b)));
                    assert_eq!(x % y, i(a.wrapping_rem(b)));
                    assert_eq!(x.div_euclid(y), i(a.wrapping_div_euclid(b)));
                    assert_eq!(x.rem_euclid(y), i(a.wrapping_rem_euclid(b)));
                    assert_eq!(x.overflowing_div_euclid(y).1, a.overflowing_div_euclid(b).1);
                    assert_eq!(
                        x.overflowing_rem_euclid(y),
                        (
                            i(a.overflowing_rem_euclid(b).0),
                            a.overflowing_rem_euclid(b).1
                        )
                    );
                }
            }
        }
    }

    #[test]
    fn test_div_rem() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(a: I, b: I)| {
                prop_assume!(!b.is_zero());
                let (q, r) = a.div_rem(b);
                assert_eq!(q * b + r, a);
                assert!(r.is_zero() || r.is_negative() == a.is_negative());
                assert!(r.unsigned_abs() < b.unsigned_abs());
                let r = a.rem_euclid(b);
                assert!(!r.is_negative());
                assert_eq!(a.div_euclid(b) * b + r, a);
            });
        });
    }

    #[test]
    fn test_i256() {
        assert_eq!(
            I256::MIN.overflowing_div(I256::MINUS_ONE),
            (I256::MIN, true)
        );
        assert_eq!(
            I256::MIN.overflowing_rem(I256::MINUS_ONE),
            (I256::ZERO, true)
        );
        assert_eq!(I256::MIN.checked_div(I256::MINUS_ONE), None);
        assert_eq!(I256::MIN.checked_div(I256::ZERO), None);
        assert_eq!(I256::MIN.checked_rem(I256::ZERO), None);
        assert_eq!(I256::MIN / I256::MAX, I256::MINUS_ONE);
        assert_eq!(I256::MIN % I256::MAX, I256::MINUS_ONE);
        assert_eq!(I256::MIN.rem_euclid(I256::MAX), I256::MAX - I256::ONE);
        assert_eq!(I256::MIN.div_euclid(I256::MAX), -I256::ONE - I256::ONE);
    }

    #[test]
    #[should_panic(expected = "attempt to divide with overflow")]
    fn test_strict_div_overflow() {
        let _ = I256::MIN.strict_div(I256::MINUS_ONE);
    }

    #[test]
    #[should_panic]
    fn test_div_by_zero() {
        let _ = I256::ONE / I256::ZERO;
    }
}
