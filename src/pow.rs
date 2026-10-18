use crate::Uint;

impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    /// Raises self to the power of `exp`.
    ///
    /// Returns None if the result would overflow.
    #[inline]
    #[must_use]
    pub const fn checked_pow(self, exp: Self) -> Option<Self> {
        match self.overflowing_pow(exp) {
            (x, false) => Some(x),
            (_, true) => None,
        }
    }

    /// Raises self to the power of `exp`, panicking if overflow occurred.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline]
    #[must_use]
    #[track_caller]
    pub const fn strict_pow(self, exp: Self) -> Self {
        match self.overflowing_pow(exp) {
            (x, false) => x,
            (_, true) => panic!("attempt to multiply with overflow"),
        }
    }

    /// Raises self to the power of `exp` and if the result would overflow.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::{Uint, uint};
    /// # uint!{
    /// assert_eq!(
    ///     36_U64.overflowing_pow(12_U64),
    ///     (0x41c21cb8e1000000_U64, false)
    /// );
    /// assert_eq!(
    ///     36_U64.overflowing_pow(13_U64),
    ///     (0x3f4c09ffa4000000_U64, true)
    /// );
    /// assert_eq!(
    ///     36_U68.overflowing_pow(13_U68),
    ///     (0x093f4c09ffa4000000_U68, false)
    /// );
    /// assert_eq!(16_U65.overflowing_pow(32_U65), (0_U65, true));
    /// # }
    /// ```
    /// Small cases:
    /// ```
    /// # use ruint::{Uint, uint};
    /// # uint!{
    /// assert_eq!(0_U0.overflowing_pow(0_U0), (0_U0, false));
    /// assert_eq!(0_U1.overflowing_pow(0_U1), (1_U1, false));
    /// assert_eq!(0_U1.overflowing_pow(1_U1), (0_U1, false));
    /// assert_eq!(1_U1.overflowing_pow(0_U1), (1_U1, false));
    /// assert_eq!(1_U1.overflowing_pow(1_U1), (1_U1, false));
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub const fn overflowing_pow(self, exp: Self) -> (Self, bool) {
        if BITS == 0 {
            return (self, false);
        }
        if self.is_small_power_of_two() {
            return self.pow_power_of_two(exp);
        }

        // Exponentiation by squaring
        let mut base = self;
        let mut exp = exp;
        let mut overflow = false;
        let mut base_overflow = false;
        let mut result = Self::ONE;
        while !exp.const_is_zero() {
            // Multiply by base
            if exp.bit(0) {
                let (r, o) = result.overflowing_mul(base);
                result = r;
                overflow |= o | base_overflow;
            }

            // Square base
            let (s, o) = base.overflowing_mul(base);
            base = s;
            base_overflow |= o;
            exp = exp.wrapping_shr(1);
        }
        (result, overflow)
    }

    /// Raises self to the power of `exp`, wrapping around on overflow.
    #[inline]
    #[must_use]
    pub const fn pow(self, exp: Self) -> Self {
        self.wrapping_pow(exp)
    }

    /// Raises self to the power of `exp`, saturating on overflow.
    #[inline]
    #[must_use]
    pub const fn saturating_pow(self, exp: Self) -> Self {
        match self.overflowing_pow(exp) {
            (x, false) => x,
            (_, true) => Self::MAX,
        }
    }

    /// Raises self to the power of `exp`, wrapping around on overflow.
    #[inline]
    #[must_use]
    pub const fn wrapping_pow(self, exp: Self) -> Self {
        if BITS == 0 {
            return self;
        }
        if self.is_small_power_of_two() {
            return self.pow_power_of_two(exp).0;
        }

        // Exponentiation by squaring
        let mut base = self;
        let mut exp = exp;
        let mut result = Self::ONE;
        while !exp.const_is_zero() {
            // Multiply by base
            if exp.bit(0) {
                result = result.wrapping_mul(base);
            }

            // Square base
            base = base.wrapping_mul(base);
            exp = exp.wrapping_shr(1);
        }
        result
    }

    /// Whether `self` is a power of two below `2^64`, like the 256 and 2 that
    /// most EVM `EXP` calls raise to a small power. Testing one limb keeps
    /// the check cheap for other bases.
    #[inline]
    const fn is_small_power_of_two(&self) -> bool {
        if !self.limbs[0].is_power_of_two() {
            return false;
        }
        // Stop at the first nonzero limb: folding them all keeps every limb
        // of a large `self` live and spills it.
        const_range_for!(i in 1..LIMBS => {
            if self.limbs[i] != 0 {
                return false;
            }
        });
        true
    }

    /// Raises `self = 2^k < 2^64` to the power of `exp` as a shift by
    /// `k * exp`, and returns whether the result overflowed.
    #[inline]
    const fn pow_power_of_two(self, exp: Self) -> (Self, bool) {
        // Exponents of `BITS` or more overflow for every base above one.
        let mut high = 0;
        const_range_for!(i in 1..LIMBS => {
            high |= exp.limbs[i];
        });
        let exp = if high == 0 && exp.limbs[0] < BITS as u64 {
            exp.limbs[0] as usize
        } else {
            BITS
        };
        let shift = exp.saturating_mul(self.limbs[0].trailing_zeros() as usize);
        (Self::ONE.wrapping_shl(shift), shift >= BITS)
    }

    /// Construct from double precision binary logarithm.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::{Uint, uint, aliases::*};
    /// # uint!{
    /// assert_eq!(U64::approx_pow2(-2.0), Some(0_U64));
    /// assert_eq!(U64::approx_pow2(-1.0), Some(1_U64));
    /// assert_eq!(U64::approx_pow2(0.0), Some(1_U64));
    /// assert_eq!(U64::approx_pow2(1.0), Some(2_U64));
    /// assert_eq!(U64::approx_pow2(1.6), Some(3_U64));
    /// assert_eq!(U64::approx_pow2(2.0), Some(4_U64));
    /// assert_eq!(U64::approx_pow2(64.0), None);
    /// assert_eq!(U64::approx_pow2(10.385), Some(1337_U64));
    /// # }
    /// ```
    #[cfg(feature = "std")]
    #[must_use]
    #[allow(clippy::missing_inline_in_public_items)]
    pub fn approx_pow2(exp: f64) -> Option<Self> {
        const LN2_1P5: f64 = 0.584_962_500_721_156_2_f64;
        const EXP2_63: f64 = 9_223_372_036_854_775_808_f64;

        // FEATURE: Round negative to zero.
        #[allow(clippy::cast_precision_loss)] // Self::BITS ~< 2^52 and so fits f64.
        if exp < LN2_1P5 {
            if exp < -1.0 {
                return Some(Self::ZERO);
            }
            return Self::try_from(1).ok();
        }
        #[allow(clippy::cast_precision_loss)]
        if exp > Self::BITS as f64 {
            return None;
        }

        // Since exp < BITS, it has an integer and fractional part.
        #[allow(clippy::cast_possible_truncation)] // exp <= BITS <= usize::MAX.
        #[allow(clippy::cast_sign_loss)] // exp >= 0.
        let shift = exp.trunc() as usize;
        let fract = exp.fract();

        // Compute the leading 64 bits
        // Since `fract < 1.0` we have `fract.exp2() < 2`, so we can rescale by
        // 2^63 and cast to u64.
        #[allow(clippy::cast_possible_truncation)] // fract < 1.0
        #[allow(clippy::cast_sign_loss)] // fract >= 0.
        let bits = (fract.exp2() * EXP2_63) as u64;
        // Note: If `fract` is zero this will result in `u64::MAX`.

        if shift >= 63 {
            // OPT: A dedicated function avoiding full-sized shift.
            Some(Self::try_from(bits).ok()?.checked_shl(shift - 63)?)
        } else {
            let shift = 63 - shift;
            // Divide `bits` by `2^shift`, rounding to nearest.
            let bits = (bits >> shift) + ((bits >> (shift - 1)) & 1);
            Self::try_from(bits).ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{const_for, nlimbs};
    use core::iter::repeat_n;
    use proptest::proptest;

    #[test]
    fn test_const_pow() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            const {
                assert!(matches!(U::MAX.checked_pow(U::ONE), Some(value) if value.const_eq(&U::MAX)));
                assert!(U::MAX.strict_pow(U::ONE).const_eq(&U::MAX));
                assert!(matches!(U::MAX.overflowing_pow(U::ONE), (value, false) if value.const_eq(&U::MAX)));
                assert!(U::MAX.pow(U::ONE).const_eq(&U::MAX));
                assert!(U::MAX.saturating_pow(U::ONE).const_eq(&U::MAX));
                assert!(U::MAX.wrapping_pow(U::ONE).const_eq(&U::MAX));
            }
        });
    }

    #[test]
    fn test_pow2_shl() {
        const_for!(BITS in NON_ZERO if BITS >= 2 {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(e in 0..=BITS+1)| {
                assert_eq!(U::from(2).pow(U::from(e)), U::from(1) << e);
            });
            proptest!(|(k in 0..BITS, e in 0..=BITS+1, high: U)| {
                let base = U::from(1) << k;
                let expected = if k * e < BITS {
                    (U::from(1) << (k * e), false)
                } else {
                    (U::ZERO, true)
                };
                assert_eq!(base.overflowing_pow(U::from(e)), expected);
                assert_eq!(base.wrapping_pow(U::from(e)), expected.0);
                // Exponents with high bits set overflow every base above one.
                let e = U::from(e) | (high << 64);
                if k > 0 && e >= U::from(BITS) {
                    assert_eq!(base.overflowing_pow(e), (U::ZERO, true));
                }
            });
        });
    }

    #[test]
    fn test_pow_product() {
        const_for!(BITS in NON_ZERO if BITS >= 64 {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(b in 2_u64..100, e in 0_usize..100)| {
                let b = U::from(b);
                let prod = repeat_n(b, e).product::<U>();
                assert_eq!(b.pow(U::from(e)), prod);
                let overflow = repeat_n(b, e).try_fold(U::from(1), U::checked_mul).is_none();
                assert_eq!(b.overflowing_pow(U::from(e)), (prod, overflow));
            });
        });
    }

    #[test]
    fn test_strict_pow_ok() {
        use crate::aliases::U64;
        assert_eq!(
            U64::from(2u64).strict_pow(U64::from(10u64)),
            U64::from(1024u64)
        );
    }

    #[test]
    #[should_panic(expected = "attempt to multiply with overflow")]
    fn test_strict_pow_overflow() {
        let _ = crate::aliases::U64::MAX.strict_pow(crate::aliases::U64::from(2u64));
    }
}
