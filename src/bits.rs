use crate::{Uint, utils::select_unpredictable_u32};
use core::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, ShlAssign, Shr,
    ShrAssign,
};

/// Saturating `usize` → `u32` cast for shift amounts.
///
/// The primitive fast paths (`LIMBS ∈ {1, 2, 4}`) feed `rhs` to
/// `unbounded_sh*`, which take a `u32`. A plain `rhs as u32` cast reduces shift
/// amounts `>= 2^32` mod `2^32`, shifting by the wrong amount instead of
/// shifting the whole value out. Saturating to `u32::MAX` keeps any such shift
/// `>= BITS`, so `unbounded_sh*` returns 0, matching the generic path.
/// Branchless, and a no-op the compiler elides on 32-bit targets (where `usize`
/// cannot exceed `u32::MAX`).
#[inline(always)]
const fn shift_amount(rhs: usize) -> u32 {
    select_unpredictable_u32(rhs > u32::MAX as usize, u32::MAX, rhs as u32)
}

impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    /// Returns whether a specific bit is set.
    ///
    /// Returns `false` if `index` exceeds the bit width of the number.
    #[must_use]
    #[inline]
    pub const fn bit(&self, index: usize) -> bool {
        if index >= BITS {
            return false;
        }
        let (limbs, bits) = (index / 64, index % 64);
        self.limbs[limbs] & (1 << bits) != 0
    }

    /// Sets a specific bit to a value.
    #[inline]
    pub const fn set_bit(&mut self, index: usize, value: bool) {
        if index >= BITS {
            return;
        }
        let (limbs, bits) = (index / 64, index % 64);
        if value {
            self.limbs[limbs] |= 1 << bits;
        } else {
            self.limbs[limbs] &= !(1 << bits);
        }
    }

    /// Returns a specific byte. The byte at index `0` is the least significant
    /// byte (little endian).
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than or equal to the byte width of the
    /// number.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::uint;
    /// let x = uint!(0x1234567890_U64);
    /// let bytes = [
    ///     x.byte(0), // 0x90
    ///     x.byte(1), // 0x78
    ///     x.byte(2), // 0x56
    ///     x.byte(3), // 0x34
    ///     x.byte(4), // 0x12
    ///     x.byte(5), // 0x00
    ///     x.byte(6), // 0x00
    ///     x.byte(7), // 0x00
    /// ];
    /// assert_eq!(bytes, x.to_le_bytes());
    /// ```
    ///
    /// Panics if out of range.
    ///
    /// ```should_panic
    /// # use ruint::uint;
    /// let x = uint!(0x1234567890_U64);
    /// let _ = x.byte(8);
    /// ```
    #[inline]
    #[must_use]
    #[track_caller]
    pub const fn byte(&self, index: usize) -> u8 {
        #[cfg(target_endian = "little")]
        {
            self.as_le_slice()[index]
        }

        #[cfg(target_endian = "big")]
        #[allow(clippy::cast_possible_truncation)] // intentional
        {
            assert!(index < Self::BYTES, "index out of bounds");
            (self.limbs[index / 8] >> ((index % 8) * 8)) as u8
        }
    }

    /// Returns a specific byte, or `None` if `index` is out of range. The byte
    /// at index `0` is the least significant byte (little endian).
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::uint;
    /// let x = uint!(0x1234567890_U64);
    /// assert_eq!(x.checked_byte(0), Some(0x90));
    /// assert_eq!(x.checked_byte(7), Some(0x00));
    /// // Out of range
    /// assert_eq!(x.checked_byte(8), None);
    /// ```
    #[inline]
    #[must_use]
    pub const fn checked_byte(&self, index: usize) -> Option<u8> {
        if index < Self::BYTES {
            Some(self.byte(index))
        } else {
            None
        }
    }

    /// Reverses the order of bits in the integer. The least significant bit
    /// becomes the most significant bit, second least-significant bit becomes
    /// second most-significant bit, etc.
    #[inline]
    #[must_use]
    pub const fn reverse_bits(mut self) -> Self {
        const_range_for!(i in 0..LIMBS / 2 => {
            let j = LIMBS - 1 - i;
            let limb = self.limbs[i];
            self.limbs[i] = self.limbs[j].reverse_bits();
            self.limbs[j] = limb.reverse_bits();
        });
        if LIMBS % 2 == 1 {
            let i = LIMBS / 2;
            self.limbs[i] = self.limbs[i].reverse_bits();
        }
        if !BITS.is_multiple_of(64) {
            self = self.wrapping_shr(64 - BITS % 64);
        }
        self
    }

    /// Inverts all the bits in the integer.
    #[inline]
    #[must_use]
    pub const fn not(mut self) -> Self {
        if BITS == 0 {
            return Self::ZERO;
        }
        const_range_for!(limb in mut self.limbs => {
            *limb = !*limb;
        });
        self.masked()
    }

    /// Returns the number of significant words (limbs) in the integer.
    ///
    /// If this is 0, then `self` is zero.
    #[inline]
    pub(crate) const fn count_significant_words(&self) -> usize {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            if self.limbs[i] != 0 {
                return i + 1;
            }
        }
        0
    }

    /// Returns the number of leading zeros in the binary representation of
    /// `self`.
    #[inline]
    #[must_use]
    pub const fn leading_zeros(&self) -> usize {
        let fixed = Self::MASK.leading_zeros() as usize;

        as_primitives!(self; {
            u64(x) => return x.leading_zeros() as usize - fixed,
            u128(x) => return x.leading_zeros() as usize - fixed,
            u256((lo, hi)) => return (select_unpredictable_u32(hi != 0,
                hi.leading_zeros(),
                lo.leading_zeros() + 128
            )) as usize - fixed,
        });

        let s = self.count_significant_words();
        if s == 0 {
            return BITS;
        }
        let n = LIMBS - s;
        let skipped = n * 64;
        let top = self.limbs[s - 1].leading_zeros() as usize;
        skipped + top - fixed
    }

    /// Returns the number of leading ones in the binary representation of
    /// `self`.
    #[inline]
    #[must_use]
    pub const fn leading_ones(&self) -> usize {
        let fixed = Self::MASK.leading_zeros() as usize;

        as_primitives!(self; {
            u64(x) => return (x | !Self::MASK).leading_ones() as usize - fixed,
            u128(x) => {
                let mask = (Self::MASK as u128) << 64 | u64::MAX as u128;
                return (x | !mask).leading_ones() as usize - fixed;
            },
            u256((lo, hi)) => {
                let hi_mask = (Self::MASK as u128) << 64 | u64::MAX as u128;
                let hi = hi | !hi_mask;
                let ones = if hi == u128::MAX {
                    hi.leading_ones() + lo.leading_ones()
                } else {
                    hi.leading_ones()
                };
                return ones as usize - fixed;
            },
        });

        Self::not(*self).leading_zeros()
    }

    /// Returns the number of trailing zeros in the binary representation of
    /// `self`.
    #[inline]
    #[must_use]
    pub const fn trailing_zeros(&self) -> usize {
        as_primitives!(self; {
            u64(x) => {
                let zeros = x.trailing_zeros() as usize;
                return if zeros > BITS { BITS } else { zeros };
            },
            u128(x) => {
                let zeros = x.trailing_zeros() as usize;
                return if zeros > BITS { BITS } else { zeros };
            },
            u256((lo, hi)) => {
                let zeros = if lo == 0 {
                    hi.trailing_zeros() + 128
                } else {
                    lo.trailing_zeros()
                } as usize;
                return if zeros > BITS { BITS } else { zeros };
            },
        });

        const_range_for!(i in 0..LIMBS => {
            if self.limbs[i] != 0 {
                return i * 64 + self.limbs[i].trailing_zeros() as usize;
            }
        });
        BITS
    }

    /// Returns the number of trailing ones in the binary representation of
    /// `self`.
    #[inline]
    #[must_use]
    pub const fn trailing_ones(&self) -> usize {
        as_primitives!(self; {
            u64(x) => return x.trailing_ones() as usize,
            u128(x) => return x.trailing_ones() as usize,
            u256((lo, hi)) => return if lo == u128::MAX {
                (hi.trailing_ones() + 128) as usize
            } else {
                lo.trailing_ones() as usize
            },
        });

        const_range_for!(i in 0..LIMBS => {
            if self.limbs[i] != u64::MAX {
                return i * 64 + self.limbs[i].trailing_ones() as usize;
            }
        });
        BITS
    }

    /// Returns the number of ones in the binary representation of `self`.
    #[inline]
    #[must_use]
    pub const fn count_ones(&self) -> usize {
        let mut ones = 0;
        const_range_for!(limb in ref self.as_limbs() => {
            ones += limb.count_ones() as usize;
        });
        ones
    }

    /// Returns the number of zeros in the binary representation of `self`.
    #[must_use]
    #[inline]
    pub const fn count_zeros(&self) -> usize {
        BITS - self.count_ones()
    }

    /// Returns the dynamic length of this number in bits, ignoring leading
    /// zeros.
    ///
    /// For the maximum length of the type, use [`Uint::BITS`](Self::BITS).
    #[must_use]
    #[inline]
    pub const fn bit_len(&self) -> usize {
        BITS - self.leading_zeros()
    }

    /// Returns the dynamic length of this number in bytes, ignoring leading
    /// zeros.
    ///
    /// For the maximum length of the type, use [`Uint::BYTES`](Self::BYTES).
    #[must_use]
    #[inline]
    pub const fn byte_len(&self) -> usize {
        self.bit_len().div_ceil(8)
    }

    /// Returns the most significant 64 bits of the number and the exponent.
    ///
    /// Given return value $(\mathtt{bits}, \mathtt{exponent})$, the `self` can
    /// be approximated as
    ///
    /// $$
    /// \mathtt{self} ≈ \mathtt{bits} ⋅ 2^\mathtt{exponent}
    /// $$
    ///
    /// If `self` is $<≥> 2^{63}$, then `exponent` will be zero and `bits` will
    /// have leading zeros.
    #[inline]
    #[must_use]
    pub const fn most_significant_bits(&self) -> (u64, usize) {
        let significant_words = self.count_significant_words();
        if significant_words == 0 {
            (0, 0)
        } else if significant_words == 1 {
            (self.limbs[0], 0)
        } else {
            let i = significant_words - 1;
            let hi = self.limbs[i];
            let lo = self.limbs[i - 1];
            let leading_zeros = hi.leading_zeros();
            let bits = if leading_zeros > 0 {
                (hi << leading_zeros) | (lo >> (64 - leading_zeros))
            } else {
                hi
            };
            let exponent = i * 64 - leading_zeros as usize;
            (bits, exponent)
        }
    }

    /// Checked left shift by `rhs` bits.
    ///
    /// Returns $\mathtt{self} ⋅ 2^{\mathtt{rhs}}$ or [`None`] if the result
    /// would $≥ 2^{\mathtt{BITS}}$. That is, it returns [`None`] if the bits
    /// shifted out would be non-zero.
    ///
    /// Note: This differs from [`u64::checked_shl`] which returns `None` if the
    /// shift is larger than BITS (which is IMHO not very useful).
    #[inline(always)]
    #[must_use]
    pub const fn checked_shl(self, rhs: usize) -> Option<Self> {
        match self.overflowing_shl(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Left shift by `rhs` bits, panicking if the bits shifted out are
    /// non-zero.
    ///
    /// Note: This differs from [`u64::strict_shl`] which panics if the shift is
    /// larger than `BITS`.
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
    /// Returns $\mathtt{self} ⋅ 2^{\mathtt{rhs}}$ or [`Uint::MAX`] if the
    /// result would $≥ 2^{\mathtt{BITS}}$. That is, it returns
    /// [`Uint::MAX`] if the bits shifted out would be non-zero.
    #[inline(always)]
    #[must_use]
    pub const fn saturating_shl(self, rhs: usize) -> Self {
        match self.overflowing_shl(rhs) {
            (value, false) => value,
            _ => Self::MAX,
        }
    }

    /// Left shift by `rhs` bits with overflow detection.
    ///
    /// Returns $\mod{\mathtt{value} ⋅ 2^{\mathtt{rhs}}}_{2^{\mathtt{BITS}}}$.
    /// If the product is $≥ 2^{\mathtt{BITS}}$ it returns `true`. That is, it
    /// returns true if the bits shifted out are non-zero.
    ///
    /// Note: This differs from [`u64::overflowing_shl`] which returns `true` if
    /// the shift is larger than `BITS` (which is IMHO not very useful).
    #[inline]
    #[must_use]
    pub const fn overflowing_shl(self, rhs: usize) -> (Self, bool) {
        let (limbs, bits) = (rhs / 64, rhs % 64);
        if limbs >= LIMBS {
            return (Self::ZERO, !self.const_is_zero());
        }
        let mut r = Self::ZERO;
        let bits = bits as u32;

        let mut carry = 0;
        // check the limbs that are entirely shifted out.
        const_range_for!(i in 0..LIMBS - limbs => {
            let x = self.limbs[i];
            r.limbs[i + limbs] = (x << bits) | carry;
            carry = x.unbounded_shr(64 - bits);
        });
        // we need to know whether any limb entirely shifted out is non-zero
        const_range_for!(i in (LIMBS - limbs)..LIMBS => {
            carry |= self.limbs[i];
        });
        // we also need to know if the top limb is dirty before masking
        carry |= r.maskable_bits();
        (r.masked(), carry != 0)
    }

    /// Left shift by `rhs` bits with overflow detection, but with `Self` rhs.
    ///
    /// See [`overflowing_shl`](Self::overflowing_shl) for details.
    #[inline]
    pub(crate) fn overflowing_shl_big(self, rhs: Self) -> (Self, bool) {
        if BITS == 0 {
            return (Self::ZERO, false);
        }
        // A shift amount that doesn't fit `usize` is `> usize::MAX >= BITS`,
        // so the entire value is shifted out. The conversion is
        // pointer-width-aware, so shift amounts in `[2^32, 2^64)` cannot
        // truncate on 32-bit targets (where `usize` is narrower than `u64`).
        let Ok(rhs) = usize::try_from(rhs) else {
            return (Self::ZERO, !self.const_is_zero());
        };
        self.overflowing_shl(rhs)
    }

    /// Left shift by `rhs` bits.
    ///
    /// Returns $\mod{\mathtt{value} ⋅ 2^{\mathtt{rhs}}}_{2^{\mathtt{BITS}}}$.
    ///
    /// Note: This differs from [`u64::wrapping_shl`] which first reduces `rhs`
    /// by `BITS` (which is IMHO not very useful).
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_shl(self, rhs: usize) -> Self {
        as_primitives!(self; {
            u64(x) => {
                let mut r = Self::ZERO;
                r.limbs[0] = x.unbounded_shl(shift_amount(rhs));
                return r.masked();
            },
            u128(x) => {
                let r = x.unbounded_shl(shift_amount(rhs));
                let mut out = Self::ZERO;
                out.limbs[0] = r as u64;
                out.limbs[1] = (r >> 64) as u64;
                return out.masked();
            },
            u256((lo, hi)) => {
                let rhs = shift_amount(rhs);
                // Compute as if rhs < 128.
                let new_lo = lo.unbounded_shl(rhs);
                let new_hi = hi.unbounded_shl(rhs) | lo.unbounded_shr(128u32.wrapping_sub(rhs));
                // If rhs >= 128, lo becomes 0 and hi becomes lo << (rhs - 128).
                let cross = lo.unbounded_shl(rhs.wrapping_sub(128));
                let mask = 0u128.wrapping_sub((rhs < 128) as u128);
                let lo = new_lo & mask;
                let hi = (new_hi & mask) | (cross & !mask);
                let mut r = Self::ZERO;
                r.limbs[0] = lo as u64;
                r.limbs[1] = (lo >> 64) as u64;
                r.limbs[2] = hi as u64;
                r.limbs[3] = (hi >> 64) as u64;
                return r.masked();
            },
        });

        self.overflowing_shl(rhs).0
    }

    /// Checked right shift by `rhs` bits.
    ///
    /// $$
    /// \frac{\mathtt{self}}{2^{\mathtt{rhs}}}
    /// $$
    ///
    /// Returns the above or [`None`] if the division is not exact. This is the
    /// same as
    ///
    /// Note: This differs from [`u64::checked_shr`] which returns `None` if the
    /// shift is larger than BITS (which is IMHO not very useful).
    #[inline(always)]
    #[must_use]
    pub const fn checked_shr(self, rhs: usize) -> Option<Self> {
        match self.overflowing_shr(rhs) {
            (value, false) => Some(value),
            _ => None,
        }
    }

    /// Right shift by `rhs` bits, panicking if the bits shifted out are
    /// non-zero.
    ///
    /// Note: This differs from [`u64::strict_shr`] which panics if the shift is
    /// larger than `BITS`.
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

    /// Right shift by `rhs` bits with underflow detection.
    ///
    /// $$
    /// \floor{\frac{\mathtt{self}}{2^{\mathtt{rhs}}}}
    /// $$
    ///
    /// Returns the above and `false` if the division was exact, and `true` if
    /// it was rounded down. This is the same as non-zero bits being shifted
    /// out.
    ///
    /// Note: This differs from [`u64::overflowing_shr`] which returns `true` if
    /// the shift is larger than `BITS` (which is IMHO not very useful).
    #[inline]
    #[must_use]
    pub const fn overflowing_shr(self, rhs: usize) -> (Self, bool) {
        let (limbs, bits) = (rhs / 64, rhs % 64);
        if limbs >= LIMBS {
            return (Self::ZERO, !self.const_is_zero());
        }
        let mut r = Self::ZERO;
        let bits = bits as u32;

        let mut carry = 0;
        // check the limbs that are entirely shifted out.
        const_range_for!(i in 0..LIMBS - limbs => {
            let x = self.limbs[LIMBS - 1 - i];
            r.limbs[LIMBS - 1 - i - limbs] = (x >> bits) | carry;
            carry = x.unbounded_shl(64 - bits);
        });
        // we need to know if any limb entirely shifted out is non-zero
        const_range_for!(i in 0..limbs => {
            carry |= self.limbs[i];
        });
        (r, carry != 0)
    }

    /// Right shift by `rhs` bits with underflow detection, but with `Self` rhs.
    ///
    /// See [`overflowing_shr`](Self::overflowing_shr) for details.
    #[inline]
    pub(crate) fn overflowing_shr_big(self, rhs: Self) -> (Self, bool) {
        if BITS == 0 {
            return (Self::ZERO, false);
        }
        // A shift amount that doesn't fit `usize` is `> usize::MAX >= BITS`,
        // so the entire value is shifted out. The conversion is
        // pointer-width-aware, so shift amounts in `[2^32, 2^64)` cannot
        // truncate on 32-bit targets (where `usize` is narrower than `u64`).
        let Ok(rhs) = usize::try_from(rhs) else {
            return (Self::ZERO, !self.const_is_zero());
        };
        self.overflowing_shr(rhs)
    }

    /// Right shift by `rhs` bits.
    ///
    /// $$
    /// \mathtt{wrapping\\_shr}(\mathtt{self}, \mathtt{rhs}) =
    /// \floor{\frac{\mathtt{self}}{2^{\mathtt{rhs}}}}
    /// $$
    ///
    /// Note: This differs from [`u64::wrapping_shr`] which first reduces `rhs`
    /// by `BITS` (which is IMHO not very useful).
    #[inline(always)]
    #[must_use]
    pub const fn wrapping_shr(self, rhs: usize) -> Self {
        as_primitives!(self; {
            u64(x) => {
                let mut r = Self::ZERO;
                r.limbs[0] = x.unbounded_shr(shift_amount(rhs));
                return r;
            },
            u128(x) => {
                let r = x.unbounded_shr(shift_amount(rhs));
                let mut out = Self::ZERO;
                out.limbs[0] = r as u64;
                out.limbs[1] = (r >> 64) as u64;
                return out;
            },
            u256((lo, hi)) => {
                let rhs = shift_amount(rhs);
                // Compute as if rhs < 128.
                let new_hi = hi.unbounded_shr(rhs);
                let new_lo = lo.unbounded_shr(rhs) | hi.unbounded_shl(128u32.wrapping_sub(rhs));
                // If rhs >= 128, hi becomes 0 and lo becomes hi >> (rhs - 128).
                let cross = hi.unbounded_shr(rhs.wrapping_sub(128));
                let mask = 0u128.wrapping_sub((rhs < 128) as u128);
                let hi = new_hi & mask;
                let lo = (new_lo & mask) | (cross & !mask);
                let mut r = Self::ZERO;
                r.limbs[0] = lo as u64;
                r.limbs[1] = (lo >> 64) as u64;
                r.limbs[2] = hi as u64;
                r.limbs[3] = (hi >> 64) as u64;
                return r;
            },
        });

        self.overflowing_shr(rhs).0
    }

    /// Arithmetic shift right by `rhs` bits.
    #[inline]
    #[must_use]
    pub const fn arithmetic_shr(self, rhs: usize) -> Self {
        if BITS == 0 {
            return Self::ZERO;
        }
        let sign = self.bit(BITS - 1);
        let mut r = self.wrapping_shr(rhs);
        if sign {
            // r |= Self::MAX << BITS.saturating_sub(rhs);
            r = r.bitor(Self::MAX.wrapping_shl(BITS.saturating_sub(rhs)));
        }
        r
    }

    /// Shifts the bits to the left by a specified amount, `rhs`, wrapping the
    /// truncated bits to the end of the resulting integer.
    #[inline]
    #[must_use]
    pub const fn rotate_left(self, rhs: usize) -> Self {
        if BITS == 0 {
            return Self::ZERO;
        }
        let rhs = rhs % BITS;
        // (self << rhs) | (self >> (BITS - rhs))
        self.wrapping_shl(rhs).bitor(self.wrapping_shr(BITS - rhs))
    }

    /// Shifts the bits to the right by a specified amount, `rhs`, wrapping the
    /// truncated bits to the beginning of the resulting integer.
    #[inline(always)]
    #[must_use]
    pub const fn rotate_right(self, rhs: usize) -> Self {
        if BITS == 0 {
            return Self::ZERO;
        }
        let rhs = rhs % BITS;
        self.rotate_left(BITS - rhs)
    }
}

impl<const BITS: usize, const LIMBS: usize> Not for Uint<BITS, LIMBS> {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        self.not()
    }
}

impl<const BITS: usize, const LIMBS: usize> Not for &Uint<BITS, LIMBS> {
    type Output = Uint<BITS, LIMBS>;

    #[inline]
    fn not(self) -> Self::Output {
        (*self).not()
    }
}

macro_rules! impl_bit_op {
    ($op:tt, $assign_op:tt, $trait:ident, $fn:ident, $trait_assign:ident, $fn_assign:ident) => {
        impl<const BITS: usize, const LIMBS: usize> $trait_assign<Uint<BITS, LIMBS>>
            for Uint<BITS, LIMBS>
        {
            #[inline(always)]
            fn $fn_assign(&mut self, rhs: Uint<BITS, LIMBS>) {
                self.$fn_assign(&rhs);
            }
        }

        impl<const BITS: usize, const LIMBS: usize> $trait_assign<&Uint<BITS, LIMBS>>
            for Uint<BITS, LIMBS>
        {
            #[inline]
            fn $fn_assign(&mut self, rhs: &Uint<BITS, LIMBS>) {
                for i in 0..LIMBS {
                    u64::$fn_assign(&mut self.limbs[i], rhs.limbs[i]);
                }
            }
        }

        impl<const BITS: usize, const LIMBS: usize> $trait<Uint<BITS, LIMBS>>
            for Uint<BITS, LIMBS>
        {
            type Output = Uint<BITS, LIMBS>;

            #[inline(always)]
            fn $fn(mut self, rhs: Uint<BITS, LIMBS>) -> Self::Output {
                self.$fn_assign(rhs);
                self
            }
        }

        impl<const BITS: usize, const LIMBS: usize> $trait<&Uint<BITS, LIMBS>>
            for Uint<BITS, LIMBS>
        {
            type Output = Uint<BITS, LIMBS>;

            #[inline(always)]
            fn $fn(mut self, rhs: &Uint<BITS, LIMBS>) -> Self::Output {
                self.$fn_assign(rhs);
                self
            }
        }

        impl<const BITS: usize, const LIMBS: usize> $trait<Uint<BITS, LIMBS>>
            for &Uint<BITS, LIMBS>
        {
            type Output = Uint<BITS, LIMBS>;

            #[inline(always)]
            fn $fn(self, mut rhs: Uint<BITS, LIMBS>) -> Self::Output {
                rhs.$fn_assign(self);
                rhs
            }
        }

        impl<const BITS: usize, const LIMBS: usize> $trait<&Uint<BITS, LIMBS>>
            for &Uint<BITS, LIMBS>
        {
            type Output = Uint<BITS, LIMBS>;

            #[inline(always)]
            fn $fn(self, rhs: &Uint<BITS, LIMBS>) -> Self::Output {
                <Uint<BITS, LIMBS>>::$fn(*self, *rhs)
            }
        }

        impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
            #[doc = concat!("Returns the bitwise `", stringify!($op), "` of the two numbers.")]
            #[inline(always)]
            #[must_use]
            pub const fn $fn(mut self, rhs: Uint<BITS, LIMBS>) -> Uint<BITS, LIMBS> {
                const_range_for!(i in 0..LIMBS => {
                    self.limbs[i] $assign_op rhs.limbs[i];
                });
                self
            }
        }
    };
}

impl_bit_op!(|, |=, BitOr,  bitor,  BitOrAssign,  bitor_assign);
impl_bit_op!(&, &=, BitAnd, bitand, BitAndAssign, bitand_assign);
impl_bit_op!(^, ^=, BitXor, bitxor, BitXorAssign, bitxor_assign);

impl<const BITS: usize, const LIMBS: usize> Shl<Self> for Uint<BITS, LIMBS> {
    type Output = Self;

    #[inline(always)]
    fn shl(self, rhs: Self) -> Self::Output {
        self.overflowing_shl_big(rhs).0
    }
}

impl<const BITS: usize, const LIMBS: usize> Shl<&Self> for Uint<BITS, LIMBS> {
    type Output = Self;

    #[inline(always)]
    fn shl(self, rhs: &Self) -> Self::Output {
        self << *rhs
    }
}

impl<const BITS: usize, const LIMBS: usize> Shr<Self> for Uint<BITS, LIMBS> {
    type Output = Self;

    #[inline(always)]
    fn shr(self, rhs: Self) -> Self::Output {
        self.overflowing_shr_big(rhs).0
    }
}

impl<const BITS: usize, const LIMBS: usize> Shr<&Self> for Uint<BITS, LIMBS> {
    type Output = Self;

    #[inline(always)]
    fn shr(self, rhs: &Self) -> Self::Output {
        self >> *rhs
    }
}

impl<const BITS: usize, const LIMBS: usize> ShlAssign<Self> for Uint<BITS, LIMBS> {
    #[inline(always)]
    fn shl_assign(&mut self, rhs: Self) {
        *self = *self << rhs;
    }
}

impl<const BITS: usize, const LIMBS: usize> ShlAssign<&Self> for Uint<BITS, LIMBS> {
    #[inline(always)]
    fn shl_assign(&mut self, rhs: &Self) {
        *self = *self << rhs;
    }
}

impl<const BITS: usize, const LIMBS: usize> ShrAssign<Self> for Uint<BITS, LIMBS> {
    #[inline(always)]
    fn shr_assign(&mut self, rhs: Self) {
        *self = *self >> rhs;
    }
}

impl<const BITS: usize, const LIMBS: usize> ShrAssign<&Self> for Uint<BITS, LIMBS> {
    #[inline(always)]
    fn shr_assign(&mut self, rhs: &Self) {
        *self = *self >> rhs;
    }
}

macro_rules! impl_shift {
    (@main $u:ty) => {
        impl<const BITS: usize, const LIMBS: usize> Shl<$u> for Uint<BITS, LIMBS> {
            type Output = Self;

            #[inline(always)]
            #[allow(clippy::cast_possible_truncation)]
            fn shl(self, rhs: $u) -> Self::Output {
                self.wrapping_shl(rhs as usize)
            }
        }

        impl<const BITS: usize, const LIMBS: usize> Shr<$u> for Uint<BITS, LIMBS> {
            type Output = Self;

            #[inline(always)]
            #[allow(clippy::cast_possible_truncation)]
            fn shr(self, rhs: $u) -> Self::Output {
                self.wrapping_shr(rhs as usize)
            }
        }
    };

    (@ref $u:ty) => {
        impl<const BITS: usize, const LIMBS: usize> Shl<&$u> for Uint<BITS, LIMBS> {
            type Output = Self;

            #[inline(always)]
            fn shl(self, rhs: &$u) -> Self::Output {
                <Self>::shl(self, *rhs)
            }
        }

        impl<const BITS: usize, const LIMBS: usize> Shr<&$u> for Uint<BITS, LIMBS> {
            type Output = Self;

            #[inline(always)]
            fn shr(self, rhs: &$u) -> Self::Output {
                <Self>::shr(self, *rhs)
            }
        }
    };

    (@assign $u:ty) => {
        impl<const BITS: usize, const LIMBS: usize> ShlAssign<$u> for Uint<BITS, LIMBS> {
            #[inline(always)]
            fn shl_assign(&mut self, rhs: $u) {
                *self = *self << rhs;
            }
        }

        impl<const BITS: usize, const LIMBS: usize> ShrAssign<$u> for Uint<BITS, LIMBS> {
            #[inline(always)]
            fn shr_assign(&mut self, rhs: $u) {
                *self = *self >> rhs;
            }
        }
    };

    ($u:ty) => {
        impl_shift!(@main $u);
        impl_shift!(@ref $u);
        impl_shift!(@assign $u);
        impl_shift!(@assign &$u);
    };

    ($u:ty, $($tail:ty),*) => {
        impl_shift!($u);
        impl_shift!($($tail),*);
    };
}

impl_shift!(usize, u8, u16, u32, isize, i8, i16, i32);

// Only when losslessy castable to usize.
#[cfg(target_pointer_width = "64")]
impl_shift!(u64, i64);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{U128, U256},
        const_for, nlimbs,
    };
    use core::cmp::min;
    use proptest::proptest;

    fn reference_leading_zeros<const BITS: usize, const LIMBS: usize>(
        value: Uint<BITS, LIMBS>,
    ) -> usize {
        let mut zeros = 0;
        while zeros < BITS && !value.bit(BITS - zeros - 1) {
            zeros += 1;
        }
        zeros
    }

    fn reference_leading_ones<const BITS: usize, const LIMBS: usize>(
        value: Uint<BITS, LIMBS>,
    ) -> usize {
        let mut ones = 0;
        while ones < BITS && value.bit(BITS - ones - 1) {
            ones += 1;
        }
        ones
    }

    fn reference_trailing_zeros<const BITS: usize, const LIMBS: usize>(
        value: Uint<BITS, LIMBS>,
    ) -> usize {
        let mut zeros = 0;
        while zeros < BITS && !value.bit(zeros) {
            zeros += 1;
        }
        zeros
    }

    fn reference_trailing_ones<const BITS: usize, const LIMBS: usize>(
        value: Uint<BITS, LIMBS>,
    ) -> usize {
        let mut ones = 0;
        while ones < BITS && value.bit(ones) {
            ones += 1;
        }
        ones
    }

    #[test]
    fn test_leading_zeros() {
        assert_eq!(Uint::<0, 0>::ZERO.leading_zeros(), 0);
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint::<BITS, LIMBS>;
            assert_eq!(U::ZERO.leading_zeros(), BITS);
            assert_eq!(U::MAX.leading_zeros(), 0);
            assert_eq!(U::ONE.leading_zeros(), BITS - 1);
            proptest!(|(value: U)| {
                assert_eq!(value.leading_zeros(), reference_leading_zeros(value));
            });
        });

        assert_eq!(
            U256::from_limbs([1, 0, 0, 0]).leading_zeros(),
            reference_leading_zeros(U256::from_limbs([1, 0, 0, 0]))
        );
        assert_eq!(
            U256::from_limbs([0, 0, 1, 0]).leading_zeros(),
            reference_leading_zeros(U256::from_limbs([0, 0, 1, 0]))
        );
    }

    #[test]
    fn test_leading_ones() {
        assert_eq!(Uint::<0, 0>::ZERO.leading_ones(), 0);
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint::<BITS, LIMBS>;
            assert_eq!(U::ZERO.leading_ones(), 0);
            assert_eq!(U::MAX.leading_ones(), BITS);
            assert_eq!((U::MAX << 1_usize).leading_ones(), BITS - 1);
            proptest!(|(value: U)| {
                assert_eq!(value.leading_ones(), reference_leading_ones(value));
            });
        });

        assert_eq!(
            U256::from_limbs([u64::MAX, u64::MAX, u64::MAX, u64::MAX - 1]).leading_ones(),
            reference_leading_ones(U256::from_limbs([
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX - 1,
            ]))
        );
        assert_eq!(
            U256::from_limbs([0, 0, u64::MAX, u64::MAX]).leading_ones(),
            reference_leading_ones(U256::from_limbs([0, 0, u64::MAX, u64::MAX]))
        );
    }

    #[test]
    fn test_trailing_zeros() {
        assert_eq!(Uint::<0, 0>::ZERO.trailing_zeros(), 0);
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint::<BITS, LIMBS>;
            assert_eq!(U::ZERO.trailing_zeros(), BITS);
            assert_eq!(U::MAX.trailing_zeros(), 0);
            assert_eq!((U::MAX << 1_usize).trailing_zeros(), 1);
            proptest!(|(value: U)| {
                assert_eq!(value.trailing_zeros(), reference_trailing_zeros(value));
            });
        });

        assert_eq!(
            U256::from_limbs([0, 0, 1, 0]).trailing_zeros(),
            reference_trailing_zeros(U256::from_limbs([0, 0, 1, 0]))
        );
    }

    #[test]
    fn test_trailing_ones() {
        assert_eq!(Uint::<0, 0>::ZERO.trailing_ones(), 0);
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint::<BITS, LIMBS>;
            assert_eq!(U::ZERO.trailing_ones(), 0);
            assert_eq!(U::MAX.trailing_ones(), BITS);
            assert_eq!((U::MAX << 1_usize).trailing_ones(), 0);
            proptest!(|(value: U)| {
                assert_eq!(value.trailing_ones(), reference_trailing_ones(value));
            });
        });

        assert_eq!(
            U256::from_limbs([u64::MAX, u64::MAX, 0, 0]).trailing_ones(),
            reference_trailing_ones(U256::from_limbs([u64::MAX, u64::MAX, 0, 0]))
        );
    }

    #[test]
    fn test_most_significant_bits() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint::<BITS, LIMBS>;
            proptest!(|(value: u64)| {
                let value = if U::LIMBS <= 1 { value & U::MASK } else { value };
                assert_eq!(U::from(value).most_significant_bits(), (value, 0));
            });
        });
        proptest!(|(mut limbs: [u64; 2])| {
            if limbs[1] == 0 {
                limbs[1] = 1;
            }
            let (bits, exponent) = U128::from_limbs(limbs).most_significant_bits();
            assert!(bits >= 1_u64 << 63);
            assert_eq!(exponent, 64 - limbs[1].leading_zeros() as usize);
        });
    }

    #[test]
    fn test_checked_shl() {
        assert_eq!(
            Uint::<65, 2>::from_limbs([0x0010_0000_0000_0000, 0]).checked_shl(1),
            Some(Uint::<65, 2>::from_limbs([0x0020_0000_0000_0000, 0]))
        );
        assert_eq!(
            Uint::<127, 2>::from_limbs([0x0010_0000_0000_0000, 0]).checked_shl(64),
            Some(Uint::<127, 2>::from_limbs([0, 0x0010_0000_0000_0000]))
        );
    }

    #[test]
    #[allow(
        clippy::cast_lossless,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    fn test_small() {
        const_for!(BITS in [1, 2, 8, 16, 32, 63, 64] {
            type U = Uint::<BITS, 1>;
            proptest!(|(a: U, b: U)| {
                assert_eq!(a | b, U::from_limbs([a.limbs[0] | b.limbs[0]]));
                assert_eq!(a & b, U::from_limbs([a.limbs[0] & b.limbs[0]]));
                assert_eq!(a ^ b, U::from_limbs([a.limbs[0] ^ b.limbs[0]]));
            });
            proptest!(|(a: U, s in 0..BITS)| {
                assert_eq!(a << s, U::from_limbs([a.limbs[0] << s & U::MASK]));
                assert_eq!(a >> s, U::from_limbs([a.limbs[0] >> s]));
            });
        });
        proptest!(|(a: Uint::<32, 1>, s in 0_usize..=34)| {
            assert_eq!(a.reverse_bits(), Uint::from((a.limbs[0] as u32).reverse_bits() as u64));
            assert_eq!(a.rotate_left(s), Uint::from((a.limbs[0] as u32).rotate_left(s as u32) as u64));
            assert_eq!(a.rotate_right(s), Uint::from((a.limbs[0] as u32).rotate_right(s as u32) as u64));
            if s < 32 {
                let arr_shifted = (((a.limbs[0] as i32) >> s) as u32) as u64;
                assert_eq!(a.arithmetic_shr(s), Uint::from_limbs([arr_shifted]));
            }
        });
        proptest!(|(a: Uint::<64, 1>, s in 0_usize..=66)| {
            assert_eq!(a.reverse_bits(), Uint::from(a.limbs[0].reverse_bits()));
            assert_eq!(a.rotate_left(s), Uint::from(a.limbs[0].rotate_left(s as u32)));
            assert_eq!(a.rotate_right(s), Uint::from(a.limbs[0].rotate_right(s as u32)));
            if s < 64 {
                let arr_shifted = ((a.limbs[0] as i64) >> s) as u64;
                assert_eq!(a.arithmetic_shr(s), Uint::from_limbs([arr_shifted]));
            }
        });
    }

    #[test]
    #[allow(clippy::absurd_extreme_comparisons)] // Generated code
    fn test_const_reverse_and_most_significant_bits() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            const {
                assert!(U::MAX.reverse_bits().const_eq(&U::MAX));
                let reversed_one = U::ONE.reverse_bits();
                let expected = if BITS == 0 { U::ZERO } else { U::ONE.wrapping_shl(BITS - 1) };
                assert!(reversed_one.const_eq(&expected));

                let (bits, exponent) = U::MAX.most_significant_bits();
                if BITS <= 64 {
                    assert!(bits == U::MASK);
                    assert!(exponent == 0);
                } else {
                    assert!(bits == u64::MAX);
                    assert!(exponent == BITS - 64);
                }
            }
        });
    }

    #[test]
    fn test_shift_reverse() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint::<BITS, LIMBS>;
            proptest!(|(value: U, shift in 0..=BITS + 2)| {
                let left = (value << shift).reverse_bits();
                let right = value.reverse_bits() >> shift;
                assert_eq!(left, right);
            });
        });
    }

    #[test]
    fn test_shift_very_big_rhs() {
        type U = Uint<128, 2>;

        for rhs in [
            U::from(u64::MAX),
            U::from(u128::MAX),
            U::from_limbs([0, 1]),
            U::from_limbs([1, 1]),
            U::from_limbs([1, u64::MAX]),
        ] {
            assert_eq!(U::ONE << rhs, U::ZERO, "{rhs}");
            assert_eq!(U::ONE >> rhs, U::ZERO, "{rhs}");
        }
    }

    #[test]
    fn test_rotate() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint::<BITS, LIMBS>;
            proptest!(|(value: U, shift in  0..=BITS + 2)| {
                let rotated = value.rotate_left(shift).rotate_right(shift);
                assert_eq!(value, rotated);
            });
        });
    }

    #[test]
    fn test_arithmetic_shr() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint::<BITS, LIMBS>;
            proptest!(|(value: U, shift in  0..=BITS + 2)| {
                let shifted = value.arithmetic_shr(shift);
                assert_eq!(shifted.leading_ones(), match value.leading_ones() {
                    0 => 0,
                    n => min(BITS, n + shift)
                });
            });
        });
    }

    #[test]
    fn test_overflowing_shr() {
        // Test: Single limb right shift from 40u64 by 1 bit.
        // Expects resulting integer: 20 with no fractional part.
        assert_eq!(
            Uint::<64, 1>::from_limbs([40u64]).overflowing_shr(1),
            (Uint::<64, 1>::from(20), false)
        );

        // Test: Single limb right shift from 41u64 by 1 bit.
        // Expects resulting integer: 20 with a detected fractional part.
        assert_eq!(
            Uint::<64, 1>::from_limbs([41u64]).overflowing_shr(1),
            (Uint::<64, 1>::from(20), true)
        );

        // Test: Two limbs right shift from 0x0010_0000_0000_0000 and 0 by 1 bit.
        // Expects resulting limbs: [0x0080_0000_0000_000, 0] with no fractional part.
        assert_eq!(
            Uint::<65, 2>::from_limbs([0x0010_0000_0000_0000, 0]).overflowing_shr(1),
            (Uint::<65, 2>::from_limbs([0x0008_0000_0000_0000, 0]), false)
        );

        // Test: Shift beyond single limb capacity with MAX value.
        // Expects the highest possible value in 256-bit representation with a detected
        // fractional part.
        assert_eq!(
            Uint::<256, 4>::MAX.overflowing_shr(65),
            (
                Uint::<256, 4>::from_str_radix(
                    "7fffffffffffffffffffffffffffffffffffffffffffffff",
                    16
                )
                .unwrap(),
                true
            )
        );
        // Test: Large 4096-bit integer right shift by 34 bits.
        // Expects a specific value with no fractional part.
        assert_eq!(
            Uint::<4096, 64>::from_str_radix("3ffffffffffffffffffffffffffffc00000000", 16,)
                .unwrap()
                .overflowing_shr(34),
            (
                Uint::<4096, 64>::from_str_radix("fffffffffffffffffffffffffffff", 16).unwrap(),
                false
            )
        );
        // Test: Extremely large 4096-bit integer right shift by 100 bits.
        // Expects a specific value with no fractional part.
        assert_eq!(
            Uint::<4096, 64>::from_str_radix(
                "fffffffffffffffffffffffffffff0000000000000000000000000",
                16,
            )
            .unwrap()
            .overflowing_shr(100),
            (
                Uint::<4096, 64>::from_str_radix("fffffffffffffffffffffffffffff", 16).unwrap(),
                false
            )
        );
        // Test: Complex 4096-bit integer right shift by 1 bit.
        // Expects a specific value with no fractional part.
        assert_eq!(
            Uint::<4096, 64>::from_str_radix(
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0bdbfe",
                16,
            )
            .unwrap()
            .overflowing_shr(1),
            (
                Uint::<4096, 64>::from_str_radix(
                    "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffff85edff",
                    16
                )
                .unwrap(),
                false
            )
        );
        // Test: Large 4096-bit integer right shift by 1000 bits.
        // Expects a specific value with no fractional part.
        assert_eq!(
            Uint::<4096, 64>::from_str_radix(
                "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                16,
            )
            .unwrap()
            .overflowing_shr(1000),
            (
                Uint::<4096, 64>::from_str_radix(
                    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                    16
                )
                .unwrap(),
                false
            )
        );
        // Test: MAX 4096-bit integer right shift by 34 bits.
        // Expects a specific value with a detected fractional part.
        assert_eq!(
            Uint::<4096, 64>::MAX
            .overflowing_shr(34),
            (
                Uint::<4096, 64>::from_str_radix(
                    "3fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                    16
                )
                .unwrap(),
                true
            )
        );
    }

    #[test]
    fn test_strict_shl_shr_ok() {
        use crate::aliases::U64;
        assert_eq!(U64::from(1u64).strict_shl(3), U64::from(8u64));
        assert_eq!(U64::from(8u64).strict_shr(3), U64::from(1u64));
    }

    #[test]
    #[should_panic(expected = "attempt to shift left with overflow")]
    fn test_strict_shl_overflow() {
        let _ = crate::aliases::U64::MAX.strict_shl(1);
    }

    #[test]
    #[should_panic(expected = "attempt to shift right with overflow")]
    fn test_strict_shr_overflow() {
        let _ = crate::aliases::U64::from(1u64).strict_shr(1);
    }

    #[test]
    fn regression_overflowing_shl() {
        // limbs entirely shifted out are caught
        let num = Uint::<128, 2>::from_limbs([0, 1]);
        assert_eq!(num.overflowing_shl(64), (Uint::ZERO, true));
        assert!(num.checked_shl(64).is_none());

        // masked bits are caught
        let num = Uint::<65, 2>::from_limbs([0, 1]);
        assert_eq!(num.overflowing_shl(1), (Uint::ZERO, true));
        assert_eq!(num.overflowing_shl(64), (Uint::ZERO, true));
    }

    #[test]
    fn regression_overflowing_shr() {
        // limbs entirely shifted out are caught
        let num = Uint::<128, 2>::from(1u64);
        assert_eq!(num.overflowing_shr(64), (Uint::ZERO, true));
        assert!(num.checked_shr(64).is_none());
    }

    // On 32-bit targets `usize` cannot exceed `u32::MAX`, so the truncation
    // this guards against cannot occur (and `1usize << 32` would not compile).
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn regression_wrapping_shifts() {
        // shift amounts >= BITS produce zero; the amount must not be
        // reduced mod 2^32 by the primitive fast paths (LIMBS = 1, 2, 4)
        let huge = 1usize << 32;
        assert_eq!(Uint::<64, 1>::from(1u64).wrapping_shl(huge), Uint::ZERO);
        assert_eq!(Uint::<64, 1>::from(1u64).wrapping_shl(huge + 3), Uint::ZERO);
        assert_eq!(Uint::<64, 1>::MAX.wrapping_shr(huge), Uint::ZERO);
        assert_eq!(Uint::<128, 2>::MAX.wrapping_shl(huge), Uint::ZERO);
        assert_eq!(Uint::<256, 4>::MAX.wrapping_shr(huge), Uint::ZERO);
        // truncating this amount lands back in range (130, cross-term 2), so
        // it exercises every internal selector of the 256-bit fast path
        assert_eq!(Uint::<256, 4>::MAX.wrapping_shl(huge + 130), Uint::ZERO);
        assert_eq!(Uint::<256, 4>::MAX.wrapping_shr(huge + 130), Uint::ZERO);
        // the generic path (3 limbs) already handles this
        assert_eq!(Uint::<192, 3>::from(1u64).wrapping_shl(huge), Uint::ZERO);

        // the operators route through wrapping_shl/wrapping_shr
        assert_eq!(Uint::<64, 1>::from(1u64) << huge, Uint::ZERO);
        assert_eq!(Uint::<64, 1>::MAX >> huge, Uint::ZERO);

        // arithmetic_shr: fills with the sign bit for huge shift amounts
        assert_eq!(Uint::<64, 1>::from(5u64).arithmetic_shr(huge), Uint::ZERO);
        assert_eq!(Uint::<64, 1>::MAX.arithmetic_shr(huge), Uint::<64, 1>::MAX);
    }

    #[test]
    fn regression_overflowing_big() {
        // The `_big` shift helpers take a `Self` shift amount and narrowed it
        // to u64, then cast to usize. On 32-bit targets that cast truncated
        // u64 -> u32, wrapping shift amounts in [2^32, 2^64) mod 2^32 (audit
        // 1.9): e.g. `U256::ONE << U256::from(1u64 << 32)` shifted by 0 and
        // returned 1. The pointer-width-aware `usize::try_from` now shifts the
        // whole value out instead, so the result is correct on every pointer
        // width. These assertions pass on 64-bit hosts and would have failed
        // on wasm32.
        type U = Uint<256, 4>;

        // The 1.9 repro amount: 2^32 >= BITS, so the whole value shifts out.
        let big = U::from(1u64 << 32);
        assert_eq!(U::ONE.overflowing_shl_big(big), (U::ZERO, true));
        assert_eq!(U::ONE.overflowing_shr_big(big), (U::ZERO, true));
        // the operators route through the `_big` helpers
        assert_eq!(U::ONE << big, U::ZERO);
        assert_eq!(U::MAX >> big, U::ZERO);

        // A shift amount of exactly BITS still shifts everything out.
        assert_eq!(U::MAX.overflowing_shl_big(U::from(256)), (U::ZERO, true));
        assert_eq!(U::MAX.overflowing_shr_big(U::from(256)), (U::ZERO, true));

        // Zero never sets the overflow flag, no matter how large the shift.
        assert_eq!(U::ZERO.overflowing_shl_big(big), (U::ZERO, false));
        assert_eq!(U::ZERO.overflowing_shr_big(big), (U::ZERO, false));

        // Shift amounts that exceed u64 take the `try_from` branch.
        let over_u64 = U::from_limbs([0, 0, 1, 0]); // 2^128 > u64::MAX
        assert_eq!(U::MAX.overflowing_shl_big(over_u64), (U::ZERO, true));
        assert_eq!(U::MAX.overflowing_shr_big(over_u64), (U::ZERO, true));
        // ... and zero still reports no overflow there.
        assert_eq!(U::ZERO.overflowing_shl_big(over_u64), (U::ZERO, false));
        assert_eq!(U::ZERO.overflowing_shr_big(over_u64), (U::ZERO, false));

        // In-range shifts (rhs < BITS) still return the real result.
        assert_eq!(
            U::ONE.overflowing_shl_big(U::from(8)),
            (U::from(256), false)
        );
        assert_eq!(
            U::from(256).overflowing_shr_big(U::from(8)),
            (U::ONE, false)
        );
    }
}
