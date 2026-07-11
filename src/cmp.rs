use crate::{Uint, algorithms};
use core::cmp::Ordering;

macro_rules! cmp_fns {
    ($($name:ident, $op:tt => |$a:ident, $b:ident| $impl:expr),* $(,)?) => {
        $(
            #[inline]
            fn $name(&self, $b: &Self) -> bool {
                let $a = self;
                as_primitives!($a, $b; {
                    u64(x, y) => return x $op y,
                    u128(x, y) => return x $op y,
                });

                $impl
            }
        )*
    };
}

impl<const BITS: usize, const LIMBS: usize> PartialOrd for Uint<BITS, LIMBS> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }

    cmp_fns! {
        lt, <  => |a, b| algorithms::lt(a.as_limbs(), b.as_limbs()),
        gt, >  => |a, b| Self::lt(b, a),
        ge, >= => |a, b| !Self::lt(a, b),
        le, <= => |a, b| !Self::lt(b, a),
    }
}

impl<const BITS: usize, const LIMBS: usize> Ord for Uint<BITS, LIMBS> {
    #[inline]
    fn cmp(&self, rhs: &Self) -> Ordering {
        as_primitives!(self, rhs; {
            u64(x, y) => return x.cmp(&y),
            u128(x, y) => return x.cmp(&y),
        });
        algorithms::cmp(self.as_limbs(), rhs.as_limbs())
    }
}

/// Implements `PartialEq` and `PartialOrd` for `Uint` and primitive integers.
///
/// This intentionally does not use `<$t>::try_from` to avoid unnecessary
/// checks for non-limb-sized primitive integers.
macro_rules! impl_for_primitives {
    ($($t:ty),* $(,)?) => {
        $(
            impl<const BITS: usize, const LIMBS: usize> PartialEq<$t> for Uint<BITS, LIMBS> {
                #[inline]
                #[allow(unused_comparisons)] // Both signed and unsigned integers use this.
                #[allow(clippy::cast_possible_truncation)] // Unreachable.
                fn eq(&self, &other: &$t) -> bool {
                    (other >= 0) & (if <$t>::BITS <= u64::BITS {
                        u64::try_from(self).ok() == Some(other as u64)
                    } else {
                        u128::try_from(self).ok() == Some(other as u128)
                    })
                }
            }

            impl<const BITS: usize, const LIMBS: usize> PartialOrd<$t> for Uint<BITS, LIMBS> {
                #[inline]
                #[allow(unused_comparisons)] // Both signed and unsigned integers use this.
                #[allow(clippy::cast_possible_truncation)] // Unreachable.
                fn partial_cmp(&self, &other: &$t) -> Option<Ordering> {
                    if other < 0 {
                        return Some(Ordering::Greater);
                    }

                    if <$t>::BITS <= u64::BITS {
                        let Ok(self_t) = u64::try_from(self) else {
                            return Some(Ordering::Greater);
                        };
                        self_t.partial_cmp(&(other as u64))
                    } else {
                        let Ok(self_t) = u128::try_from(self) else {
                            return Some(Ordering::Greater);
                        };
                        self_t.partial_cmp(&(other as u128))
                    }
                }
            }
        )*
    };
}

#[rustfmt::skip]
impl_for_primitives!(
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
);

/// `const_eq` XOR-folds the limbs up to this many limbs, and falls back to a
/// limb-counting loop above it. LLVM lowers the fold by context: when the
/// result feeds a dependency chain and the operands live in registers it
/// stays scalar (`xor`/`or` on x86-64, `ccmp` flags on aarch64), avoiding
/// the vector round-trip that stalls dependent use — reloading the
/// consumer's 8-byte scalar stores as 16-byte vector loads defeats
/// store-to-load forwarding (measured 44% faster per link than the
/// double-word form on Zen 2 at 256 bits); standalone it re-vectorizes at
/// 8+ limbs (`pxor`/`ptest` on x86-64, NEON on aarch64). At 4 limbs x86-64
/// keeps even the standalone form scalar, costing ~0.3 ns against `ptest`
/// but still beating the derived `PartialEq`. At high limb counts the
/// serial forms lose to LLVM's auto-vectorization of the counting loop
/// (measured cliff at 24 limbs on Zen 2, crossover by 64 limbs on Apple
/// M-series).
///
/// The win is latency-shaped: instruction-count profiling (e.g. CodSpeed)
/// is blind to both the stall and the fix, so only wall-clock benchmarks of
/// dependency-chained use can catch a regression here.
const EQ_FOLD_MAX_LIMBS: usize = 16;

impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    /// Returns `true` if the value is zero.
    #[inline]
    #[must_use]
    pub fn is_zero(&self) -> bool {
        if LIMBS <= EQ_FOLD_MAX_LIMBS {
            self.const_is_zero()
        } else {
            // Comparing against the constant `ZERO` lowers to `bcmp` against
            // an all-zeros constant, which beats every const-compatible
            // formulation at high limb counts but is not reachable from
            // `const fn`.
            *self == Self::ZERO
        }
    }

    /// Returns `true` if the value is zero.
    ///
    /// This is equivalent to [`is_zero`](Self::is_zero), usable in `const`
    /// contexts. With more than `EQ_FOLD_MAX_LIMBS` limbs this may perform
    /// worse than [`is_zero`](Self::is_zero), which can compare against the
    /// zero constant with `memcmp`.
    #[inline]
    #[must_use]
    pub const fn const_is_zero(&self) -> bool {
        if LIMBS <= EQ_FOLD_MAX_LIMBS {
            self.const_eq(&Self::ZERO)
        } else {
            // An OR-fold auto-vectorizes into a plain vector reduction with
            // no comparisons, which wins at high limb counts. Below the
            // cutoff the same auto-vectorization costs a vector->scalar
            // domain crossing that `const_eq`'s chunked form avoids.
            let mut acc = 0u64;
            const_range_for!(limb in ref self.as_limbs() => {
                acc |= *limb;
            });
            acc == 0
        }
    }

    /// Returns `true` if `self` equals `other`.
    ///
    /// This is equivalent to the derived `PartialEq` (`==` operator), usable
    /// in `const` contexts.
    #[inline]
    #[must_use]
    pub const fn const_eq(&self, other: &Self) -> bool {
        // TODO: Replace with `self == other` and deprecate once `PartialEq` is const.
        as_primitives!(self, other; {
            u64(x, y) => return x == y,
            u128(x, y) => return x == y,
        });
        if LIMBS <= EQ_FOLD_MAX_LIMBS {
            let a = self.as_limbs();
            let b = other.as_limbs();
            let mut acc = 0;
            const_range_for!(i in 0..LIMBS => {
                acc |= a[i] ^ b[i];
            });
            acc == 0
        } else {
            let a = self.as_limbs();
            let b = other.as_limbs();
            let mut equal_count = 0usize;
            const_range_for!(i in 0..LIMBS => {
                equal_count += (a[i] == b[i]) as usize;
            });
            equal_count == LIMBS
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Uint, const_for, nlimbs};
    use core::cmp::Ordering;
    use proptest::{prop_assert, prop_assert_eq, proptest};

    fn reference_cmp<const BITS: usize, const LIMBS: usize>(
        a: &Uint<BITS, LIMBS>,
        b: &Uint<BITS, LIMBS>,
    ) -> Ordering {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            match a.as_limbs()[i].cmp(&b.as_limbs()[i]) {
                Ordering::Equal => {}
                non_eq => return non_eq,
            }
        }
        Ordering::Equal
    }

    fn check_cmp<const BITS: usize, const LIMBS: usize>(
        a: Uint<BITS, LIMBS>,
        b: Uint<BITS, LIMBS>,
    ) -> Result<(), proptest::prelude::TestCaseError> {
        let cmp = reference_cmp(&a, &b);
        prop_assert_eq!(a.cmp(&b), cmp);
        prop_assert_eq!(a < b, cmp.is_lt());
        prop_assert_eq!(a > b, cmp.is_gt());
        prop_assert_eq!(a <= b, !cmp.is_gt());
        prop_assert_eq!(a >= b, !cmp.is_lt());
        Ok(())
    }

    #[test]
    fn test_is_zero() {
        assert!(Uint::<0, 0>::ZERO.is_zero());
        assert!(Uint::<1, 1>::ZERO.is_zero());
        assert!(Uint::<7, 1>::ZERO.is_zero());
        assert!(Uint::<64, 1>::ZERO.is_zero());

        assert!(!Uint::<1, 1>::from_limbs([1]).is_zero());
        assert!(!Uint::<7, 1>::from_limbs([1]).is_zero());
        assert!(!Uint::<64, 1>::from_limbs([1]).is_zero());
    }

    #[test]
    fn test_cmp() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, b: U)| {
                check_cmp(a, b)?;
            });
        });
    }

    #[test]
    fn test_const_eq_ctfe() {
        const OK: bool = {
            let a = Uint::<256, 4>::from_limbs([1, 2, 3, 4]);
            let b = Uint::<256, 4>::from_limbs([1, 2, 3, 5]);
            let c = Uint::<192, 3>::from_limbs([1, 2, 3]);
            let d = Uint::<192, 3>::from_limbs([1, 2, 4]);
            // 32 limbs exercises the above-`EQ_FOLD_MAX_LIMBS` fallback
            // branches (counting loop and OR-fold) in CTFE.
            type Wide = Uint<2048, 32>;
            a.const_eq(&a)
                && !a.const_eq(&b)
                && !a.const_is_zero()
                && Uint::<256, 4>::ZERO.const_is_zero()
                && c.const_eq(&c)
                && !c.const_eq(&d)
                && Wide::ZERO.const_is_zero()
                && !Wide::MAX.const_is_zero()
                && Wide::MAX.const_eq(&Wide::MAX)
                && !Wide::MAX.const_eq(&Wide::ZERO)
        };
        const { assert!(OK) };
    }

    #[test]
    fn test_const_eq() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            assert!(U::ZERO.const_eq(&U::ZERO));
            assert!(U::ZERO.const_is_zero());
            assert!(U::MAX.const_eq(&U::MAX));
            assert_eq!(U::MAX.const_is_zero(), U::MAX.is_zero());
            proptest!(|(a: U, b: U)| {
                prop_assert_eq!(a.const_eq(&b), a == b);
                prop_assert!(a.const_eq(&a));
                prop_assert_eq!(a.const_is_zero(), a.is_zero());
            });
        });
        // `SIZES` jumps from 8 to 64 limbs; pin both sides of the
        // `EQ_FOLD_MAX_LIMBS` cutoff (16 and 17 limbs).
        const_for!(BITS in [1024, 1088] {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            assert!(U::ZERO.const_is_zero());
            assert!(U::MAX.const_eq(&U::MAX));
            proptest!(|(a: U, b: U)| {
                prop_assert_eq!(a.const_eq(&b), a == b);
                prop_assert!(a.const_eq(&a));
                prop_assert_eq!(a.const_is_zero(), a.is_zero());
            });
        });
        // A difference in any single limb must be detected.
        const_for!(BITS in NON_ZERO {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(a: U, limb in 0..LIMBS)| {
                let mut b = a;
                // Bit 0 of every limb is always within the mask.
                unsafe { b.as_limbs_mut()[limb] ^= 1 };
                prop_assert!(!a.const_eq(&b));
                prop_assert!(!b.const_eq(&a));
            });
        });
    }
}
