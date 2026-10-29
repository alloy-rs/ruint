use crate::Int;
use core::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, ShlAssign, Shr,
    ShrAssign,
};

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// Returns whether bit `index` of the two's complement representation is
    /// set.
    ///
    /// Returns `false` if `index` exceeds the bit width of the number.
    #[inline]
    #[must_use]
    pub const fn bit(&self, index: usize) -> bool {
        self.0.bit(index)
    }

    /// Sets bit `index` of the two's complement representation to `value`.
    ///
    /// Does nothing if `index` exceeds the bit width of the number.
    #[inline]
    pub const fn set_bit(&mut self, index: usize, value: bool) {
        self.0.set_bit(index, value);
    }

    /// Returns byte `index` of the two's complement representation, least
    /// significant first.
    ///
    /// # Panics
    ///
    /// Panics if `index` exceeds the byte width of the number.
    #[inline]
    #[must_use]
    #[track_caller]
    pub const fn byte(&self, index: usize) -> u8 {
        self.0.byte(index)
    }

    /// Returns byte `index` of the two's complement representation, least
    /// significant first, or [`None`] if `index` exceeds the byte width of the
    /// number.
    #[inline]
    #[must_use]
    pub const fn checked_byte(&self, index: usize) -> Option<u8> {
        self.0.checked_byte(index)
    }

    /// Reverses the order of bits in the two's complement representation.
    #[inline]
    #[must_use]
    pub const fn reverse_bits(self) -> Self {
        Self(self.0.reverse_bits())
    }

    /// Inverts all the bits in the integer, which is the same as `-self - 1`.
    #[inline]
    #[must_use]
    pub const fn not(self) -> Self {
        Self(self.0.not())
    }

    /// Computes the bitwise or of `self` and `rhs`.
    #[inline]
    #[must_use]
    pub const fn bitor(self, rhs: Self) -> Self {
        Self(self.0.bitor(rhs.0))
    }

    /// Computes the bitwise and of `self` and `rhs`.
    #[inline]
    #[must_use]
    pub const fn bitand(self, rhs: Self) -> Self {
        Self(self.0.bitand(rhs.0))
    }

    /// Computes the bitwise xor of `self` and `rhs`.
    #[inline]
    #[must_use]
    pub const fn bitxor(self, rhs: Self) -> Self {
        Self(self.0.bitxor(rhs.0))
    }

    /// Returns the number of leading zeros in the two's complement
    /// representation.
    #[inline]
    #[must_use]
    pub const fn leading_zeros(&self) -> usize {
        self.0.leading_zeros()
    }

    /// Returns the number of leading ones in the two's complement
    /// representation.
    #[inline]
    #[must_use]
    pub const fn leading_ones(&self) -> usize {
        self.0.leading_ones()
    }

    /// Returns the number of trailing zeros in the two's complement
    /// representation.
    #[inline]
    #[must_use]
    pub const fn trailing_zeros(&self) -> usize {
        self.0.trailing_zeros()
    }

    /// Returns the number of trailing ones in the two's complement
    /// representation.
    #[inline]
    #[must_use]
    pub const fn trailing_ones(&self) -> usize {
        self.0.trailing_ones()
    }

    /// Returns the number of ones in the two's complement representation.
    #[inline]
    #[must_use]
    pub const fn count_ones(&self) -> usize {
        self.0.count_ones()
    }

    /// Returns the number of zeros in the two's complement representation.
    #[inline]
    #[must_use]
    pub const fn count_zeros(&self) -> usize {
        self.0.count_zeros()
    }

    /// Checked left shift by `rhs` bits.
    ///
    /// Returns $\mathtt{self} ⋅ 2^{\mathtt{rhs}}$ or [`None`] if it does not
    /// fit.
    ///
    /// Note: This differs from [`i64::checked_shl`] which returns `None` if
    /// the shift is larger than `BITS`.
    #[inline(always)]
    #[must_use]
    pub const fn checked_shl(self, rhs: usize) -> Option<Self> {
        match self.overflowing_shl(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Left shift by `rhs` bits, panicking if $\mathtt{self} ⋅
    /// 2^{\mathtt{rhs}}$ does not fit.
    ///
    /// Note: This differs from [`i64::strict_shl`] which panics if the shift
    /// is larger than `BITS`.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn strict_shl(self, rhs: usize) -> Self {
        match self.overflowing_shl(rhs) {
            (value, false) => value,
            _ => panic!("attempt to shift left with overflow"),
        }
    }

    /// Saturating left shift by `rhs` bits.
    ///
    /// Returns $\mathtt{self} ⋅ 2^{\mathtt{rhs}}$, saturating at the numeric
    /// bounds if it does not fit.
    #[inline(always)]
    #[must_use]
    pub const fn saturating_shl(self, rhs: usize) -> Self {
        match self.overflowing_shl(rhs) {
            (value, false) => value,
            _ if self.is_negative() => Self::MIN,
            _ => Self::MAX,
        }
    }

    /// Left shift by `rhs` bits with overflow detection.
    ///
    /// Returns $\mod{\mathtt{self} ⋅ 2^{\mathtt{rhs}}}_{2^{\mathtt{BITS}}}$
    /// and whether $\mathtt{self} ⋅ 2^{\mathtt{rhs}}$ does not fit. That is,
    /// it returns `true` if any of the bits shifted out, or the resulting sign
    /// bit, differ from the sign bit of `self`.
    ///
    /// Note: This differs from [`i64::overflowing_shl`] which returns `true`
    /// if the shift is larger than `BITS`.
    #[inline]
    #[must_use]
    pub const fn overflowing_shl(self, rhs: usize) -> (Self, bool) {
        let sign_bits = if self.is_negative() {
            self.0.leading_ones()
        } else {
            self.0.leading_zeros()
        };
        let overflow = sign_bits <= rhs && !self.0.const_is_zero();
        (self.wrapping_shl(rhs), overflow)
    }

    /// Left shift by `rhs` bits.
    ///
    /// Returns $\mod{\mathtt{self} ⋅ 2^{\mathtt{rhs}}}_{2^{\mathtt{BITS}}}$.
    ///
    /// Note: This differs from [`i64::wrapping_shl`] which first reduces `rhs`
    /// by `BITS`.
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_shl(self, rhs: usize) -> Self {
        Self(self.0.wrapping_shl(rhs))
    }

    /// Checked arithmetic right shift by `rhs` bits.
    ///
    /// Returns $\frac{\mathtt{self}}{2^{\mathtt{rhs}}}$ or [`None`] if the
    /// division is not exact.
    ///
    /// Note: This differs from [`i64::checked_shr`] which returns `None` if
    /// the shift is larger than `BITS`.
    #[inline(always)]
    #[must_use]
    pub const fn checked_shr(self, rhs: usize) -> Option<Self> {
        match self.overflowing_shr(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Arithmetic right shift by `rhs` bits, panicking if the bits shifted
    /// out are non-zero.
    ///
    /// Note: This differs from [`i64::strict_shr`] which panics if the shift
    /// is larger than `BITS`.
    ///
    /// # Panics
    ///
    /// This function will always panic on overflow, regardless of whether
    /// overflow checks are enabled.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const fn strict_shr(self, rhs: usize) -> Self {
        match self.overflowing_shr(rhs) {
            (value, false) => value,
            _ => panic!("attempt to shift right with overflow"),
        }
    }

    /// Arithmetic right shift by `rhs` bits with underflow detection.
    ///
    /// Returns $\floor{\frac{\mathtt{self}}{2^{\mathtt{rhs}}}}$ and `false` if
    /// the division was exact, and `true` if it was rounded down. This is the
    /// same as non-zero bits being shifted out.
    ///
    /// Note: This differs from [`i64::overflowing_shr`] which returns `true`
    /// if the shift is larger than `BITS`.
    #[inline]
    #[must_use]
    pub const fn overflowing_shr(self, rhs: usize) -> (Self, bool) {
        let shifted_out = if rhs < BITS { rhs } else { BITS };
        (
            self.wrapping_shr(rhs),
            self.0.trailing_zeros() < shifted_out,
        )
    }

    /// Arithmetic right shift by `rhs` bits.
    ///
    /// Returns $\floor{\frac{\mathtt{self}}{2^{\mathtt{rhs}}}}$, which is `0`
    /// or `-1` if `rhs` is at least `BITS`.
    ///
    /// Note: This differs from [`i64::wrapping_shr`] which first reduces `rhs`
    /// by `BITS`.
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_shr(self, rhs: usize) -> Self {
        Self(self.0.arithmetic_shr(rhs))
    }

    /// Shifts the bits to the left by a specified amount, `rhs`, wrapping the
    /// truncated bits to the end of the resulting integer.
    #[inline]
    #[must_use]
    pub const fn rotate_left(self, rhs: usize) -> Self {
        Self(self.0.rotate_left(rhs))
    }

    /// Shifts the bits to the right by a specified amount, `rhs`, wrapping the
    /// truncated bits to the beginning of the resulting integer.
    #[inline]
    #[must_use]
    pub const fn rotate_right(self, rhs: usize) -> Self {
        Self(self.0.rotate_right(rhs))
    }
}

impl<const BITS: usize, const LIMBS: usize> Not for Int<BITS, LIMBS> {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self::not(self)
    }
}

impl<const BITS: usize, const LIMBS: usize> Not for &Int<BITS, LIMBS> {
    type Output = Int<BITS, LIMBS>;

    #[inline]
    fn not(self) -> Self::Output {
        Int::not(*self)
    }
}

impl_bin_op!(Int, BitOr, bitor, BitOrAssign, bitor_assign, bitor);
impl_bin_op!(Int, BitAnd, bitand, BitAndAssign, bitand_assign, bitand);
impl_bin_op!(Int, BitXor, bitxor, BitXorAssign, bitxor_assign, bitxor);

impl_shift!(Int; usize, u8, u16, u32, u64, u128, isize, i8, i16, i32, i64, i128);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{I8, I64, I256},
        const_for, nlimbs,
    };
    use proptest::proptest;

    #[test]
    fn test_small_exhaustive() {
        for a in i8::MIN..=i8::MAX {
            let x = I8::try_from(a).unwrap();
            let i = |v: i8| I8::try_from(v).unwrap();
            assert_eq!(!x, i(!a));
            assert_eq!(x.leading_zeros(), a.leading_zeros() as usize);
            assert_eq!(x.trailing_ones(), a.trailing_ones() as usize);
            for rhs in 0..12 {
                let wide = i32::from(a) << rhs;
                let shl = i8::try_from(wide).ok();
                assert_eq!(x.checked_shl(rhs), shl.map(i));
                assert_eq!(
                    x.wrapping_shl(rhs),
                    i(a.checked_shl(rhs as u32).unwrap_or(0))
                );
                assert_eq!(
                    x.saturating_shl(rhs),
                    i(shl.unwrap_or(if a < 0 { i8::MIN } else { i8::MAX }))
                );
                let shr = a.checked_shr(rhs as u32).unwrap_or(a >> 7);
                assert_eq!(x >> rhs, i(shr));
                assert_eq!(
                    x.checked_shr(rhs),
                    (i32::from(shr) << rhs == i32::from(a)).then(|| i(shr))
                );
                assert_eq!(x.rotate_left(rhs), i(a.rotate_left(rhs as u32)));
                assert_eq!(x.rotate_right(rhs), i(a.rotate_right(rhs as u32)));
            }
        }
    }

    #[test]
    fn test_i64_reference() {
        proptest!(|(a: i64, b: i64, rhs in 0usize..64)| {
            let i = |v: i64| I64::try_from(v).unwrap();
            let (x, y) = (i(a), i(b));
            assert_eq!(x & y, i(a & b));
            assert_eq!(x | y, i(a | b));
            assert_eq!(x ^ y, i(a ^ b));
            assert_eq!(x << rhs, i(a << rhs));
            assert_eq!(x >> rhs, i(a >> rhs));
            assert_eq!(x.reverse_bits(), i(a.reverse_bits()));
            assert_eq!(x.count_ones(), a.count_ones() as usize);
        });
    }

    #[test]
    fn test_shift_roundtrip() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(a: I, rhs in 0usize..=BITS + 1)| {
                if let Some(shifted) = a.checked_shl(rhs) {
                    assert_eq!(shifted >> rhs, a);
                }
                if let Some(shifted) = a.checked_shr(rhs) {
                    assert_eq!(shifted << rhs, a);
                }
                assert_eq!((a >> rhs).is_negative(), a.is_negative());
            });
        });
    }

    #[test]
    fn test_i256() {
        assert_eq!(I256::MINUS_ONE >> 1000, I256::MINUS_ONE);
        assert_eq!(I256::MIN >> 255, I256::MINUS_ONE);
        assert_eq!(I256::MAX >> 255, I256::ZERO);
        assert_eq!(I256::MINUS_ONE >> -1, I256::MINUS_ONE);
        assert_eq!(I256::MINUS_ONE << -1, I256::ZERO);
        assert_eq!(I256::MINUS_ONE.checked_shl(255), Some(I256::MIN));
        assert_eq!(I256::MINUS_ONE.checked_shl(256), None);
        assert_eq!(I256::ONE.checked_shl(255), None);
        assert_eq!(I256::ZERO.checked_shl(1000), Some(I256::ZERO));
        assert_eq!(I256::MIN.checked_shr(255), Some(I256::MINUS_ONE));
        assert_eq!(I256::MIN.checked_shr(256), None);
        assert_eq!(I256::ONE.saturating_shl(255), I256::MAX);
        assert_eq!(I256::MINUS_ONE.saturating_shl(256), I256::MIN);
    }
}
