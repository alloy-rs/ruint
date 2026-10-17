use crate::{Uint, algorithms};

// FEATURE: sub_mod, neg_mod, div_mod, root_mod
// See <https://en.wikipedia.org/wiki/Cipolla's_algorithm>
// FEATURE: Barrett reduction
// See also <https://static1.squarespace.com/static/61f7cacf2d7af938cad5b81c/t/62deb4e0c434f7134c2730ee/1658762465114/modular_multiplication.pdf>
// FEATURE: Modular wrapper class, like Wrapping.

impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    /// ⚠️ Compute $\mod{\mathtt{self}}_{\mathtt{modulus}}$.
    ///
    /// **Warning.** This function is not part of the stable API.
    ///
    /// Returns zero if the modulus is zero.
    // FEATURE: Reduce larger bit-sizes to smaller ones.
    #[inline]
    #[must_use]
    pub fn reduce_mod(mut self, modulus: Self) -> Self {
        if modulus.is_zero() {
            return Self::ZERO;
        }
        if self >= modulus {
            self %= modulus;
        }
        self
    }

    /// Compute $\mod{\mathtt{self} + \mathtt{rhs}}_{\mathtt{modulus}}$.
    ///
    /// Returns zero if the modulus is zero.
    #[inline]
    #[must_use]
    pub fn add_mod(self, rhs: Self, mut modulus: Self) -> Self {
        // Fast path for a modulus with a nonzero high limb and operands whose
        // high limbs do not exceed it, which implies that the operands are less
        // than twice the modulus. This covers already reduced operands.
        if LIMBS >= 2
            && modulus.limbs[LIMBS - 1] != 0
            && self.limbs[LIMBS - 1] <= modulus.limbs[LIMBS - 1]
            && rhs.limbs[LIMBS - 1] <= modulus.limbs[LIMBS - 1]
        {
            let reduce = |x: Self| {
                let (r, borrow) = x.overflowing_sub(modulus);
                if borrow { x } else { r }
            };
            let (sum, carry) = reduce(self).overflowing_add(reduce(rhs));
            let (r, borrow) = sum.overflowing_sub(modulus);
            return if carry || !borrow { r } else { sum };
        }

        self.add_mod_by_ref(&rhs, &mut modulus);
        modulus
    }

    #[inline(never)]
    fn add_mod_by_ref(&self, rhs: &Self, modulus: &mut Self) {
        if modulus.is_zero() {
            return;
        }

        // This is not going to truncate with the final cast because the modulus
        // value is 64 bits.
        #[allow(clippy::cast_possible_truncation)]
        if BITS <= 64 {
            modulus.limbs[0] =
                ((self.limbs[0] as u128 + rhs.limbs[0] as u128) % modulus.limbs[0] as u128) as u64;
            return;
        }

        // do overflowing add, then check if we should divrem
        let (result, overflow) = self.overflowing_add(*rhs);
        if overflow {
            // Add carry bit to the result. We might need an extra limb.
            let_double_bits!(numerator);
            let (limb, bit) = (BITS / 64, BITS % 64);
            let numerator = &mut numerator[..=limb];
            numerator[..LIMBS].copy_from_slice(result.as_limbs());
            numerator[limb] |= 1 << bit;

            // Reuse `div_rem` if we don't need an extra limb.
            if const { crate::nlimbs(BITS + 1) == LIMBS } {
                let numerator = unsafe { &mut *numerator.as_mut_ptr().cast::<Self>() };
                Self::div_rem_by_ref(numerator, modulus);
            } else {
                Self::div_rem_bits_plus_one(numerator.as_mut_ptr(), modulus);
            }
        } else {
            *modulus = result.reduce_mod(*modulus);
        }
    }

    #[inline(never)]
    fn div_rem_bits_plus_one(numerator: *mut u64, modulus: &mut Self) {
        // TODO(dani): check if this is worth special casing over just using
        // div_rem_double_bits
        let numerator = unsafe { core::slice::from_raw_parts_mut(numerator, LIMBS + 1) };
        algorithms::div::div_inlined(numerator, &mut modulus.limbs);
    }

    /// Compute $\mod{\mathtt{self} ⋅ \mathtt{rhs}}_{\mathtt{modulus}}$.
    ///
    /// Returns zero if the modulus is zero.
    ///
    /// See [`mul_redc`](Self::mul_redc) for a faster variant at the cost of
    /// some pre-computation.
    #[inline(always)]
    #[must_use]
    pub fn mul_mod(self, rhs: Self, mut modulus: Self) -> Self {
        self.mul_mod_by_ref(&rhs, &mut modulus);
        modulus
    }

    #[inline(never)]
    fn mul_mod_by_ref(&self, rhs: &Self, modulus: &mut Self) {
        if modulus.is_zero() {
            return;
        }

        // Since `2^BITS = 1 (mod 2^BITS - 1)`, reduce the product by adding its
        // high half to its low half with an end-around carry.
        if BITS.is_multiple_of(64) && *modulus == Self::MAX {
            let Product(lo, hi, _) = Product::new(&self.limbs, &rhs.limbs);
            let (sum, carry) = Self::from_limbs(lo).overflowing_add(Self::from_limbs(hi));
            let sum = sum.wrapping_add(Self::from(carry));
            *modulus = if sum == Self::MAX { Self::ZERO } else { sum };
            return;
        }

        // Fast path for operands with nonzero high limbs, which is the common
        // case for field arithmetic.
        if LIMBS >= 2
            && self.limbs[LIMBS - 1] != 0
            && rhs.limbs[LIMBS - 1] != 0
            && modulus.limbs[LIMBS - 1] != 0
        {
            let mut product = Product::new(&self.limbs, &rhs.limbs);
            // SAFETY: The modulus has at least two limbs and a nonzero high
            // limb, and the product has more limbs than the modulus, with a
            // zero high limb.
            unsafe {
                algorithms::div::rem_nxm_spare_limb(product.as_mut_slice(), &mut modulus.limbs)
            };
            return;
        }

        let_double_bits!(product);
        let overflow = algorithms::addmul(product, self.as_limbs(), rhs.as_limbs());
        debug_assert!(!overflow);
        Self::div_rem_double_bits(product, modulus);
    }

    #[inline]
    fn div_rem_double_bits(numerator: &mut [u64], modulus: &mut Self) {
        assume!(numerator.len() == crate::nlimbs(BITS * 2));
        algorithms::div::div_inlined(numerator, &mut modulus.limbs);
    }

    /// Compute $\mod{\mathtt{self}^{\mathtt{rhs}}}_{\mathtt{modulus}}$.
    ///
    /// Returns zero if the modulus is zero.
    #[inline]
    #[must_use]
    pub fn pow_mod(mut self, mut exp: Self, modulus: Self) -> Self {
        if BITS == 0 || modulus <= Self::ONE {
            return Self::ZERO;
        }

        // Exponentiation by squaring
        let mut result = Self::ONE;
        while exp > Self::ZERO {
            // Multiply by base
            if exp.limbs[0] & 1 == 1 {
                result = result.mul_mod(self, modulus);
            }

            // Square base
            self = self.mul_mod(self, modulus);
            exp >>= 1;
        }
        result
    }

    /// Compute $\mod{\mathtt{self}^{-1}}_{\mathtt{modulus}}$.
    ///
    /// Returns `None` if the inverse does not exist.
    #[inline]
    #[must_use]
    pub fn inv_mod(self, modulus: Self) -> Option<Self> {
        algorithms::inv_mod(self, modulus)
    }

    /// Montgomery multiplication.
    ///
    /// Requires `self` and `other` to be less than `modulus`.
    ///
    /// Computes
    ///
    /// $$
    /// \mod{\frac{\mathtt{self} ⋅ \mathtt{other}}{ 2^{64 ·
    /// \mathtt{LIMBS}}}}_{\mathtt{modulus}} $$
    ///
    /// This is useful because it can be computed notably faster than
    /// [`mul_mod`](Self::mul_mod). Many computations can be done by
    /// pre-multiplying values with $R = 2^{64 · \mathtt{LIMBS}}$
    /// and then using [`mul_redc`](Self::mul_redc) instead of
    /// [`mul_mod`](Self::mul_mod).
    ///
    /// For this algorithm to work, it needs an extra parameter `inv` which must
    /// be set to
    ///
    /// $$
    /// \mathtt{inv} = \mod{\frac{-1}{\mathtt{modulus}} }_{2^{64}}
    /// $$
    ///
    /// The `inv` value only exists for odd values of `modulus`. It can be
    /// computed using [`inv_ring`](Self::inv_ring) from `U64`.
    ///
    /// ```
    /// # use ruint::{uint, Uint, aliases::*};
    /// # uint!{
    /// # let modulus = 21888242871839275222246405745257275088548364400416034343698204186575808495617_U256;
    /// let inv = U64::wrapping_from(modulus).inv_ring().unwrap().wrapping_neg().to();
    /// let prod = 5_U256.mul_redc(6_U256, modulus, inv);
    /// # assert_eq!(inv.wrapping_mul(modulus.wrapping_to()), u64::MAX);
    /// # assert_eq!(inv, 0xc2e1f593efffffff);
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `inv` is not correct in debug mode.
    #[inline]
    #[must_use]
    pub fn mul_redc(self, other: Self, modulus: Self, inv: u64) -> Self {
        if BITS == 0 {
            return Self::ZERO;
        }
        let result = algorithms::mul_redc(self.limbs, other.limbs, modulus.limbs, inv);
        let result = Self::from_limbs(result);
        debug_assert!(result < modulus);
        result
    }

    /// Montgomery squaring.
    ///
    /// See [Self::mul_redc].
    #[inline]
    #[must_use]
    pub fn square_redc(self, modulus: Self, inv: u64) -> Self {
        if BITS == 0 {
            return Self::ZERO;
        }
        let result = algorithms::square_redc(self.limbs, modulus.limbs, inv);
        let result = Self::from_limbs(result);
        debug_assert!(result < modulus);
        result
    }
}

/// Double-width product plus a spare limb for normalization.
#[repr(C)]
struct Product<const LIMBS: usize>([u64; LIMBS], [u64; LIMBS], u64);

impl<const LIMBS: usize> Product<LIMBS> {
    /// Fixed-size schoolbook multiplication. All sizes are known at compile
    /// time, which lets the compiler unroll the loops.
    #[inline(always)]
    fn new(a: &[u64; LIMBS], b: &[u64; LIMBS]) -> Self {
        let mut product = Self([0; LIMBS], [0; LIMBS], 0);
        let limbs = product.as_mut_slice();
        for (j, &b) in b.iter().enumerate() {
            // SAFETY: Both slices have `LIMBS` limbs.
            limbs[j + LIMBS] = unsafe { algorithms::addmul_nx1(&mut limbs[j..j + LIMBS], a, b) };
        }
        product
    }

    #[inline(always)]
    fn as_mut_slice(&mut self) -> &mut [u64] {
        // SAFETY: `Self` is `repr(C)` and consists of `2 * LIMBS + 1` `u64`s,
        // so it has no padding.
        unsafe { core::slice::from_raw_parts_mut((&raw mut *self).cast::<u64>(), 2 * LIMBS + 1) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{aliases::U64, const_for, nlimbs};
    use proptest::{prop_assume, proptest, test_runner::Config};

    #[test]
    fn test_commutative() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, b: U, m: U)| {
                assert_eq!(a.mul_mod(b, m), b.mul_mod(a, m));
            });
        });
    }

    #[test]
    fn test_associative() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, b: U, c: U, m: U)| {
                assert_eq!(a.mul_mod(b.mul_mod(c, m), m), a.mul_mod(b, m).mul_mod(c, m));
            });
        });
    }

    #[test]
    fn test_distributive() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, b: U, c: U, m: U)| {
                assert_eq!(a.mul_mod(b.add_mod(c, m), m), a.mul_mod(b, m).add_mod(a.mul_mod(c, m), m));
            });
        });
    }

    #[test]
    fn test_add_identity() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(value: U, m: U)| {
                assert_eq!(value.add_mod(U::from(0), m), value.reduce_mod(m));
            });
        });
    }

    #[test]
    fn test_mul_identity() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(value: U, m: U)| {
                assert_eq!(value.mul_mod(U::from(0), m), U::ZERO);
                assert_eq!(value.mul_mod(U::from(1), m), value.reduce_mod(m));
            });
        });
    }

    #[test]
    fn test_add_mod_widening() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            const BITS_W: usize = BITS + 1;
            const LIMBS_W: usize = nlimbs(BITS_W);
            type U = Uint<BITS, LIMBS>;
            type W = Uint<BITS_W, LIMBS_W>;
            proptest!(|(a: U, b: U, m: U, shift in 0..BITS)| {
                let m = m >> shift;
                // Cover reduced operands and operands just above the modulus.
                let (ar, br) = (a.reduce_mod(m), b.reduce_mod(m));
                for (a, b) in [(a, b), (ar, br), (m.saturating_add(ar), br), (ar, m.saturating_add(br))] {
                    let expected = if m.is_zero() {
                        U::ZERO
                    } else {
                        U::from((W::from(a) + W::from(b)) % W::from(m))
                    };
                    assert_eq!(a.add_mod(b, m), expected);
                }
            });
        });
    }

    #[test]
    fn test_mul_mod_widening() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            const BITS_W: usize = 2 * BITS;
            const LIMBS_W: usize = nlimbs(BITS_W);
            type U = Uint<BITS, LIMBS>;
            type W = Uint<BITS_W, LIMBS_W>;
            proptest!(|(a: U, b: U, m: U, shift in 0..BITS)| {
                // Vary the modulus size to cover every normalization shift.
                for (a, m) in [(a, m >> shift), (a, U::MAX), (a >> shift, U::MAX)] {
                    let expected = if m.is_zero() {
                        U::ZERO
                    } else {
                        U::from(a.widening_mul::<BITS, LIMBS, BITS_W, LIMBS_W>(b) % W::from(m))
                    };
                    assert_eq!(a.mul_mod(b, m), expected);
                }
            });
            assert_eq!(U::MAX.mul_mod(U::ONE, U::MAX), U::ZERO);
            assert_eq!(U::MAX.mul_mod(U::MAX, U::MAX), U::ZERO);
        });
    }

    #[test]
    fn test_pow_identity() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, m: U)| {
                assert_eq!(a.pow_mod(U::from(0), m), U::from(1).reduce_mod(m));
                assert_eq!(a.pow_mod(U::from(1), m), a.reduce_mod(m));
            });
        });
    }

    #[test]
    fn test_pow_rules() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;

            // Too slow.
            if LIMBS > 8 {
                return;
            }

            let config = Config { cases: 5, ..Default::default() };
            proptest!(config, |(a: U, b: U, c: U, m: U)| {
                // TODO: a^(b+c) = a^b * a^c. Which requires carmichael fn.
                // TODO: (a^b)^c = a^(b * c). Which requires carmichael fn.
                assert_eq!(a.mul_mod(b, m).pow_mod(c, m), a.pow_mod(c, m).mul_mod(b.pow_mod(c, m), m));
            });
        });
    }

    #[test]
    fn test_inv() {
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, m: U)| {
                if let Some(inv) = a.inv_mod(m) {
                    assert_eq!(a.mul_mod(inv, m), U::from(1));
                }
            });
        });
    }

    #[test]
    fn test_mul_redc() {
        const_for!(BITS in NON_ZERO if BITS >= 16 {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, b: U, m: U)| {
                prop_assume!(m >= U::from(2));
                if let Some(inv) = U64::from(m.as_limbs()[0]).inv_ring() {
                    let inv = (-inv).as_limbs()[0];

                    let r = U::from(2).pow_mod(U::from(64 * LIMBS), m);
                    let ar = a.mul_mod(r, m);
                    let br = b.mul_mod(r, m);
                    // TODO: Test for larger (>= m) values of a, b.

                    let expected = a.mul_mod(b, m).mul_mod(r, m);

                    assert_eq!(ar.mul_redc(br, m, inv), expected);
                }
            });
        });
    }

    #[test]
    fn test_square_redc() {
        const_for!(BITS in NON_ZERO if BITS >= 16 {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, m: U)| {
                prop_assume!(m >= U::from(2));
                if let Some(inv) = U64::from(m.as_limbs()[0]).inv_ring() {
                    let inv = (-inv).as_limbs()[0];

                    let r = U::from(2).pow_mod(U::from(64 * LIMBS), m);
                    let ar = a.mul_mod(r, m);
                    // TODO: Test for larger (>= m) values of a, b.

                    let expected = a.mul_mod(a, m).mul_mod(r, m);

                    assert_eq!(ar.square_redc(m, inv), expected);
                }
            });
        });
    }
}
