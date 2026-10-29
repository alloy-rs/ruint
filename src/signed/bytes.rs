use crate::{Int, Uint};

#[cfg(feature = "alloc")]
#[allow(unused_imports)]
use alloc::vec::Vec;

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// The size of this integer type in bytes. If `BITS` is not divisible by
    /// eight, the most significant bits of the byte representation are copies
    /// of the sign bit.
    pub const BYTES: usize = Uint::<BITS, LIMBS>::BYTES;

    /// Converts `self` to a little-endian two's complement byte array of size
    /// exactly [`Self::BYTES`].
    ///
    /// # Panics
    ///
    /// Panics if the generic parameter `BYTES` is not exactly [`Self::BYTES`].
    /// Ideally this would be a compile time error, but this is blocked by
    /// Rust issue [#60551].
    ///
    /// [#60551]: https://github.com/rust-lang/rust/issues/60551
    #[must_use]
    #[inline]
    pub const fn to_le_bytes<const BYTES: usize>(&self) -> [u8; BYTES] {
        let mut bytes = self.0.to_le_bytes::<BYTES>();
        if BYTES > 0 {
            bytes[BYTES - 1] |= self.sign_padding();
        }
        bytes
    }

    /// Converts `self` to a little-endian two's complement byte vector of size
    /// exactly [`Self::BYTES`].
    #[cfg(feature = "alloc")]
    #[must_use]
    #[inline]
    pub fn to_le_bytes_vec(&self) -> Vec<u8> {
        let mut bytes = self.0.to_le_bytes_vec();
        if let Some(last) = bytes.last_mut() {
            *last |= self.sign_padding();
        }
        bytes
    }

    /// Converts `self` to a big-endian two's complement byte array of size
    /// exactly [`Self::BYTES`].
    ///
    /// # Panics
    ///
    /// Panics if the generic parameter `BYTES` is not exactly [`Self::BYTES`].
    /// Ideally this would be a compile time error, but this is blocked by
    /// Rust issue [#60551].
    ///
    /// [#60551]: https://github.com/rust-lang/rust/issues/60551
    #[must_use]
    #[inline]
    pub const fn to_be_bytes<const BYTES: usize>(&self) -> [u8; BYTES] {
        let mut bytes = self.0.to_be_bytes::<BYTES>();
        if BYTES > 0 {
            bytes[0] |= self.sign_padding();
        }
        bytes
    }

    /// Converts `self` to a big-endian two's complement byte vector of size
    /// exactly [`Self::BYTES`].
    #[cfg(feature = "alloc")]
    #[must_use]
    #[inline]
    pub fn to_be_bytes_vec(&self) -> Vec<u8> {
        let mut bytes = self.0.to_be_bytes_vec();
        if let Some(first) = bytes.first_mut() {
            *first |= self.sign_padding();
        }
        bytes
    }

    /// Converts a little-endian two's complement byte array of size exactly
    /// [`Self::BYTES`].
    ///
    /// # Panics
    ///
    /// Panics if the generic parameter `BYTES` is not exactly [`Self::BYTES`].
    /// Ideally this would be a compile time error, but this is blocked by
    /// Rust issue [#60551].
    ///
    /// [#60551]: https://github.com/rust-lang/rust/issues/60551
    ///
    /// Panics if the bits above `BITS` are not copies of the sign bit.
    #[must_use]
    #[track_caller]
    #[inline]
    pub const fn from_le_bytes<const BYTES: usize>(bytes: [u8; BYTES]) -> Self {
        const { Uint::<BITS, LIMBS>::assert_bytes(BYTES) }
        Self::from_le_slice(&bytes)
    }

    /// Creates a new integer from a little-endian two's complement slice of
    /// bytes.
    ///
    /// The slice must be at most [`Self::BYTES`] long, and is sign-extended if
    /// it is shorter. An empty slice is zero.
    ///
    /// # Panics
    ///
    /// Panics if the value does not fit in the [`Int`].
    #[must_use]
    #[track_caller]
    #[inline]
    pub const fn from_le_slice(bytes: &[u8]) -> Self {
        match Self::try_from_le_slice(bytes) {
            Some(value) => value,
            None => panic!("Value too large for Int"),
        }
    }

    /// Creates a new integer from a little-endian two's complement slice of
    /// bytes.
    ///
    /// The slice must be at most [`Self::BYTES`] long, and is sign-extended if
    /// it is shorter. An empty slice is zero.
    ///
    /// Returns [`None`] if the value does not fit in the [`Int`].
    #[must_use]
    #[inline]
    pub const fn try_from_le_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > Self::BYTES {
            return None;
        }
        if BITS.is_multiple_of(8) && bytes.len() == Self::BYTES {
            // Every full-width byte string is a valid value.
            return match Uint::try_from_le_slice(bytes) {
                Some(value) => Some(Self(value)),
                None => None,
            };
        }
        let [.., top] = *bytes else {
            return Some(Self::ZERO);
        };
        let mut limbs = [if top >= 0x80 { u64::MAX } else { 0 }; LIMBS];
        const_range_for!(i in 0..bytes.len() => {
            set_byte(&mut limbs, i, bytes[i]);
        });
        Self::from_sign_extended_limbs(limbs)
    }

    /// Converts a big-endian two's complement byte array of size exactly
    /// [`Self::BYTES`].
    ///
    /// # Panics
    ///
    /// Panics if the generic parameter `BYTES` is not exactly [`Self::BYTES`].
    /// Ideally this would be a compile time error, but this is blocked by
    /// Rust issue [#60551].
    ///
    /// [#60551]: https://github.com/rust-lang/rust/issues/60551
    ///
    /// Panics if the bits above `BITS` are not copies of the sign bit.
    #[must_use]
    #[track_caller]
    #[inline]
    pub const fn from_be_bytes<const BYTES: usize>(bytes: [u8; BYTES]) -> Self {
        const { Uint::<BITS, LIMBS>::assert_bytes(BYTES) }
        Self::from_be_slice(&bytes)
    }

    /// Creates a new integer from a big-endian two's complement slice of
    /// bytes.
    ///
    /// The slice must be at most [`Self::BYTES`] long, and is sign-extended if
    /// it is shorter. An empty slice is zero.
    ///
    /// # Panics
    ///
    /// Panics if the value does not fit in the [`Int`].
    #[must_use]
    #[track_caller]
    #[inline]
    pub const fn from_be_slice(bytes: &[u8]) -> Self {
        match Self::try_from_be_slice(bytes) {
            Some(value) => value,
            None => panic!("Value too large for Int"),
        }
    }

    /// Creates a new integer from a big-endian two's complement slice of
    /// bytes.
    ///
    /// The slice must be at most [`Self::BYTES`] long, and is sign-extended if
    /// it is shorter. An empty slice is zero.
    ///
    /// Returns [`None`] if the value does not fit in the [`Int`].
    #[must_use]
    #[inline]
    pub const fn try_from_be_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > Self::BYTES {
            return None;
        }
        if BITS.is_multiple_of(8) && bytes.len() == Self::BYTES {
            // Every full-width byte string is a valid value.
            return match Uint::try_from_be_slice(bytes) {
                Some(value) => Some(Self(value)),
                None => None,
            };
        }
        let [top, ..] = *bytes else {
            return Some(Self::ZERO);
        };
        let mut limbs = [if top >= 0x80 { u64::MAX } else { 0 }; LIMBS];
        const_range_for!(i in 0..bytes.len() => {
            set_byte(&mut limbs, i, bytes[bytes.len() - 1 - i]);
        });
        Self::from_sign_extended_limbs(limbs)
    }

    /// The bits above `BITS` in the most significant byte of the byte
    /// representation.
    #[inline]
    const fn sign_padding(&self) -> u8 {
        if self.is_negative() && !BITS.is_multiple_of(8) {
            0xff << (BITS % 8)
        } else {
            0
        }
    }

    /// Creates an [`Int`] from limbs whose bits above `BITS` must be copies of
    /// the sign bit.
    #[inline]
    const fn from_sign_extended_limbs(mut limbs: [u64; LIMBS]) -> Option<Self> {
        if LIMBS == 0 {
            return Some(Self::ZERO);
        }
        // All-zeros or all-ones if the bits from the sign bit up are equal.
        let high = limbs[LIMBS - 1] as i64 >> ((BITS - 1) % 64);
        if high != 0 && high != -1 {
            return None;
        }
        limbs[LIMBS - 1] &= Uint::<BITS, LIMBS>::MASK;
        Some(Self::from_limbs(limbs))
    }
}

/// Sets byte `index` of `limbs`, least significant first.
#[inline]
const fn set_byte<const LIMBS: usize>(limbs: &mut [u64; LIMBS], index: usize, byte: u8) {
    let (limb, shift) = (index / 8, index % 8 * 8);
    limbs[limb] = (limbs[limb] & !(0xff << shift)) | ((byte as u64) << shift);
}

#[cfg(test)]
mod tests {
    use crate::{
        Int,
        aliases::{I0, I8, I64, I256},
        const_for, nlimbs,
    };
    use proptest::proptest;

    #[test]
    fn test_i64_reference() {
        proptest!(|(a: i64, len in 0usize..=8)| {
            let x = I64::try_from(a).unwrap();
            assert_eq!(x.to_le_bytes::<8>(), a.to_le_bytes());
            assert_eq!(x.to_be_bytes::<8>(), a.to_be_bytes());
            assert_eq!(I64::from_le_bytes(a.to_le_bytes()), x);
            assert_eq!(I64::from_be_bytes(a.to_be_bytes()), x);
            // The low `len` bytes, sign-extended.
            let low = if len == 0 { 0 } else { a << (64 - len * 8) >> (64 - len * 8) };
            let low = Some(I64::try_from(low).unwrap());
            assert_eq!(I64::try_from_le_slice(&a.to_le_bytes()[..len]), low);
            assert_eq!(I64::try_from_be_slice(&a.to_be_bytes()[8 - len..]), low);
        });
    }

    #[test]
    #[allow(clippy::absurd_extreme_comparisons)] // From macro.
    fn test_roundtrip() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            const BYTES: usize = Int::<BITS, LIMBS>::BYTES;
            type I = Int<BITS, LIMBS>;
            proptest!(|(value: I)| {
                let le = value.to_le_bytes::<BYTES>();
                let be = value.to_be_bytes::<BYTES>();
                assert_eq!(I::from_le_bytes(le), value);
                assert_eq!(I::from_be_bytes(be), value);
                assert_eq!(le.iter().rev().copied().collect::<alloc::vec::Vec<_>>(), be);
                assert_eq!(value.to_le_bytes_vec(), le);
                assert_eq!(value.to_be_bytes_vec(), be);
                assert_eq!(I256::try_from_le_slice(&le), (BITS <= 256).then(|| I256::from(value)));
            });
        });
    }

    #[test]
    fn test_edge_cases() {
        type I4 = Int<4, 1>;
        assert_eq!(I0::try_from_le_slice(&[]), Some(I0::ZERO));
        assert_eq!(I0::try_from_le_slice(&[0]), None);
        assert_eq!(I8::try_from_be_slice(&[]), Some(I8::ZERO));
        assert_eq!(I8::try_from_be_slice(&[0, 0]), None);
        assert_eq!(I4::MINUS_ONE.to_le_bytes(), [0xff]);
        assert_eq!(I4::MIN.to_be_bytes(), [0xf8]);
        assert_eq!(I4::MAX.to_be_bytes(), [0x07]);
        assert_eq!(I4::try_from_le_slice(&[0xf8]), Some(I4::MIN));
        assert_eq!(I4::try_from_le_slice(&[0x08]), None);
        assert_eq!(I4::try_from_le_slice(&[0xf7]), None);
        assert_eq!(I256::try_from_be_slice(&[0xff]), Some(I256::MINUS_ONE));
        assert_eq!(
            I256::try_from_be_slice(&[0x00, 0x80]),
            Some(I256::try_from(0x80).unwrap())
        );
        assert_eq!(
            I256::try_from_le_slice(&[0x00, 0x80]),
            Some(I256::try_from(-0x8000).unwrap())
        );
        assert_eq!(I256::try_from_be_slice(&[0; 33]), None);
    }
}
