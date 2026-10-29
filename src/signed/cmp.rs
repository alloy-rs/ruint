use crate::Int;
use core::cmp::Ordering;

impl<const BITS: usize, const LIMBS: usize> PartialOrd for Int<BITS, LIMBS> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const BITS: usize, const LIMBS: usize> Ord for Int<BITS, LIMBS> {
    #[inline]
    fn cmp(&self, rhs: &Self) -> Ordering {
        // Values of the same sign compare like their two's complement
        // representations.
        match (self.is_negative(), rhs.is_negative()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => self.0.cmp(&rhs.0),
        }
    }
}

/// Implements `PartialEq` and `PartialOrd` for `Int` and primitive integers.
macro_rules! impl_for_primitives {
    ($($t:ty),* $(,)?) => {
        $(
            impl<const BITS: usize, const LIMBS: usize> PartialEq<$t> for Int<BITS, LIMBS> {
                #[inline]
                fn eq(&self, other: &$t) -> bool {
                    <$t>::try_from(self).ok() == Some(*other)
                }
            }

            impl<const BITS: usize, const LIMBS: usize> PartialOrd<$t> for Int<BITS, LIMBS> {
                #[inline]
                fn partial_cmp(&self, other: &$t) -> Option<Ordering> {
                    // Values that do not fit in the primitive are beyond its
                    // bounds.
                    Some(match <$t>::try_from(self) {
                        Ok(value) => value.cmp(other),
                        Err(_) if self.is_negative() => Ordering::Less,
                        Err(_) => Ordering::Greater,
                    })
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

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// Returns `true` if the value is zero.
    #[inline]
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Returns `true` if the value is zero.
    ///
    /// Note that this currently might perform worse than
    /// [`is_zero`](Self::is_zero).
    #[inline]
    #[must_use]
    pub const fn const_is_zero(&self) -> bool {
        self.0.const_is_zero()
    }

    /// Returns `true` if `self` equals `other`.
    ///
    /// Note that this currently might perform worse than the derived
    /// `PartialEq` (`==` operator).
    #[inline]
    #[must_use]
    pub const fn const_eq(&self, other: &Self) -> bool {
        self.0.const_eq(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Int,
        aliases::{I8, I256},
        const_for, nlimbs,
    };
    use core::cmp::Ordering;
    use proptest::proptest;

    #[test]
    fn test_small_exhaustive() {
        for a in i8::MIN..=i8::MAX {
            let x = I8::try_from(a).unwrap();
            for b in i8::MIN..=i8::MAX {
                let y = I8::try_from(b).unwrap();
                assert_eq!(x.cmp(&y), a.cmp(&b));
                assert_eq!(x.partial_cmp(&b), Some(a.cmp(&b)));
                assert_eq!(x == b, a == b);
            }
            assert_eq!(x.partial_cmp(&200u8), Some(Ordering::Less));
            assert_eq!(x.partial_cmp(&i128::MIN), Some(Ordering::Greater));
        }
    }

    #[test]
    fn test_cmp() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(a: I, b: I)| {
                assert_eq!(a.cmp(&b), b.cmp(&a).reverse());
                // Flipping the sign bit maps signed order onto unsigned order.
                let biased = |x: I| (x ^ I::MIN).cast_unsigned();
                assert_eq!(a.cmp(&b), biased(a).cmp(&biased(b)));
                assert!(I::MIN <= a && a <= I::MAX);
            });
        });
    }

    #[test]
    fn test_primitives() {
        assert!(I256::MIN < i128::MIN);
        assert!(I256::MAX > u128::MAX);
        assert!(I256::MINUS_ONE < 0u8);
        assert_eq!(I256::MINUS_ONE, -1i8);
        assert_ne!(I256::MINUS_ONE, u8::MAX);
        assert_eq!(I256::try_from(u128::MAX).unwrap(), u128::MAX);
    }
}
