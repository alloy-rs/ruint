use crate::{Int, Sign, Uint};

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// Raises `self` to the power of `exp`, returning [`None`] on overflow.
    #[inline]
    #[must_use]
    pub const fn checked_pow(self, exp: Uint<BITS, LIMBS>) -> Option<Self> {
        match self.overflowing_pow(exp) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Raises `self` to the power of `exp`, panicking on overflow.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline]
    #[must_use]
    #[track_caller]
    pub const fn strict_pow(self, exp: Uint<BITS, LIMBS>) -> Self {
        match self.overflowing_pow(exp) {
            (value, false) => value,
            _ => panic!("attempt to multiply with overflow"),
        }
    }

    /// Raises `self` to the power of `exp`.
    ///
    /// Returns a tuple of the exponentiation along with a boolean indicating
    /// whether an overflow happened. If an overflow would have occurred then
    /// the wrapped value is returned.
    #[inline]
    #[must_use]
    pub const fn overflowing_pow(self, exp: Uint<BITS, LIMBS>) -> (Self, bool) {
        let (abs, overflow) = self.unsigned_abs().overflowing_pow(exp);
        // Negating the wrapped absolute value gives the wrapped power.
        let (value, sign_overflow) = Self::overflowing_from_sign_and_abs(self.pow_sign(exp), abs);
        (value, overflow || sign_overflow)
    }

    /// Raises `self` to the power of `exp`, wrapping around on overflow.
    #[inline]
    #[must_use]
    pub const fn pow(self, exp: Uint<BITS, LIMBS>) -> Self {
        self.wrapping_pow(exp)
    }

    /// Raises `self` to the power of `exp`, saturating at the numeric bounds
    /// on overflow.
    #[inline]
    #[must_use]
    pub const fn saturating_pow(self, exp: Uint<BITS, LIMBS>) -> Self {
        match self.overflowing_pow(exp) {
            (value, false) => value,
            _ => match self.pow_sign(exp) {
                Sign::Positive => Self::MAX,
                Sign::Negative => Self::MIN,
            },
        }
    }

    /// Raises `self` to the power of `exp`, wrapping around on overflow.
    #[inline]
    #[must_use]
    pub const fn wrapping_pow(self, exp: Uint<BITS, LIMBS>) -> Self {
        Self(self.0.wrapping_pow(exp))
    }

    /// The sign of `self` to the power of `exp`.
    #[inline(always)]
    const fn pow_sign(self, exp: Uint<BITS, LIMBS>) -> Sign {
        if self.is_negative() && exp.bit(0) {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{I8, I64, I256, U8, U64, U256},
        const_for, nlimbs,
    };
    use proptest::proptest;

    #[test]
    fn test_i64_reference() {
        proptest!(|(a: i64, exp in 0u32..130)| {
            let i = |v: i64| I64::try_from(v).unwrap();
            let x = i(a);
            let e = U64::from(exp);
            assert_eq!(x.overflowing_pow(e), (i(a.overflowing_pow(exp).0), a.overflowing_pow(exp).1));
            assert_eq!(x.saturating_pow(e), i(a.saturating_pow(exp)));
            assert_eq!(x.pow(e), i(a.wrapping_pow(exp)));
        });
    }

    #[test]
    fn test_small_exhaustive() {
        for a in i8::MIN..=i8::MAX {
            for exp in 0..=u8::MAX {
                let i = |v: i8| I8::try_from(v).unwrap();
                let (x, e) = (i(a), U8::from(exp));
                let exp = u32::from(exp);
                assert_eq!(x.checked_pow(e), a.checked_pow(exp).map(i));
                assert_eq!(x.saturating_pow(e), i(a.saturating_pow(exp)));
                assert_eq!(x.wrapping_pow(e), i(a.wrapping_pow(exp)));
            }
        }
    }

    #[test]
    fn test_pow_consistent() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: I, exp in 0u64..8)| {
                let e = U::wrapping_from(exp);
                let product = (0..e.to::<u64>()).fold(U::ONE, |p, _| p * a.cast_unsigned());
                assert_eq!(a.overflowing_pow(e).0.cast_unsigned(), product);
            });
        });
    }

    #[test]
    fn test_i256() {
        let two = I256::ONE + I256::ONE;
        assert_eq!(two.pow(U256::from(255)), I256::MIN);
        assert_eq!(two.overflowing_pow(U256::from(255)), (I256::MIN, true));
        assert_eq!((-two).overflowing_pow(U256::from(255)), (I256::MIN, false));
        assert_eq!((-two).saturating_pow(U256::from(256)), I256::MAX);
        assert_eq!((-two).saturating_pow(U256::from(257)), I256::MIN);
        assert_eq!(I256::MINUS_ONE.pow(U256::MAX), I256::MINUS_ONE);
        assert_eq!(I256::MINUS_ONE.checked_pow(U256::ZERO), Some(I256::ONE));
    }
}
