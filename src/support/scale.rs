//! Support for the [`parity-scale-codec`](https://crates.io/crates/parity-scale-codec) crate.

#![cfg(feature = "parity-scale-codec")]
#![cfg_attr(docsrs, doc(cfg(feature = "parity-scale-codec")))]

use crate::Uint;
use parity_scale_codec::{
    Compact, CompactAs, Decode, Encode, EncodeAsRef, EncodeLike, Error, HasCompact, Input,
    MaxEncodedLen, Output,
};

#[allow(unused_imports)]
use alloc::vec::Vec;

// Compact encoding is supported only for 0-(2**536-1) values:
// https://docs.substrate.io/reference/scale-codec/#fn-1
const COMPACT_BITS_LIMIT: usize = 536;

impl<const BITS: usize, const LIMBS: usize> Encode for Uint<BITS, LIMBS> {
    /// u32 prefix for compact encoding + bytes needed for LE bytes
    /// representation
    fn size_hint(&self) -> usize {
        core::mem::size_of::<u32>() + Self::BYTES
    }

    fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
        self.as_le_bytes().using_encoded(f)
    }
}

impl<const BITS: usize, const LIMBS: usize> MaxEncodedLen for Uint<BITS, LIMBS> {
    fn max_encoded_len() -> usize {
        core::mem::size_of::<Self>()
    }
}

impl<const BITS: usize, const LIMBS: usize> Decode for Uint<BITS, LIMBS> {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        Decode::decode(input).and_then(|b: Vec<_>| {
            Self::try_from_le_slice(&b)
                .ok_or_else(|| Error::from("value is larger than fits the Uint"))
        })
    }
}

pub struct CompactUint<const BITS: usize, const LIMBS: usize>(pub Uint<BITS, LIMBS>);

impl<const BITS: usize, const LIMBS: usize> From<Uint<BITS, LIMBS>> for CompactUint<BITS, LIMBS> {
    fn from(v: Uint<BITS, LIMBS>) -> Self {
        Self(v)
    }
}

impl<const BITS: usize, const LIMBS: usize> From<CompactUint<BITS, LIMBS>> for Uint<BITS, LIMBS> {
    fn from(v: CompactUint<BITS, LIMBS>) -> Self {
        v.0
    }
}

impl<const BITS: usize, const LIMBS: usize> From<Compact<Self>> for CompactUint<BITS, LIMBS> {
    fn from(v: Compact<Self>) -> Self {
        v.0
    }
}

impl<const BITS: usize, const LIMBS: usize> CompactAs for CompactUint<BITS, LIMBS> {
    type As = Uint<BITS, LIMBS>;

    fn encode_as(&self) -> &Self::As {
        &self.0
    }

    fn decode_from(v: Self::As) -> Result<Self, Error> {
        Ok(Self(v))
    }
}

impl<const BITS: usize, const LIMBS: usize> HasCompact for Uint<BITS, LIMBS> {
    type Type = CompactUint<BITS, LIMBS>;
}

pub struct CompactRefUint<'a, const BITS: usize, const LIMBS: usize>(pub &'a Uint<BITS, LIMBS>);

impl<'a, const BITS: usize, const LIMBS: usize> From<&'a Uint<BITS, LIMBS>>
    for CompactRefUint<'a, BITS, LIMBS>
{
    fn from(v: &'a Uint<BITS, LIMBS>) -> Self {
        Self(v)
    }
}

impl<'a, const BITS: usize, const LIMBS: usize> EncodeAsRef<'a, Uint<BITS, LIMBS>>
    for CompactUint<BITS, LIMBS>
{
    type RefType = CompactRefUint<'a, BITS, LIMBS>;
}

impl<const BITS: usize, const LIMBS: usize> EncodeLike for CompactRefUint<'_, BITS, LIMBS> {}

/// Compact/general integers are encoded with the two least significant bits
/// denoting the mode:
/// * `0b00`: single-byte mode: upper six bits are the LE encoding of the value.
///   Valid only for values of `0-63`.
/// * `0b01`: two-byte  mode: upper six bits and the following byte is the LE
///   encoding of the value. Valid only for values of `64-(2\*\*14-1)`.
/// * `0b10`: four-byte mode: upper  six bits and the following three bytes are
///   the LE encoding of the value. Valid only for values of
///   `(2\*\*14)-(2\*\*30-1)`.
/// * `0b11`: Big-integer mode: The upper six bits are the number of bytes
///   following, plus four. The  value is contained, LE encoded, in the bytes
///   following. The final (most  significant) byte must be non-zero. Valid only
///   for values of `(2\*\*30)-(2\*\*536-1)`.
impl<const BITS: usize, const LIMBS: usize> Encode for CompactRefUint<'_, BITS, LIMBS> {
    fn size_hint(&self) -> usize {
        match self.0.bit_len() {
            0..=6 => 1,
            7..=14 => 2,
            15..=30 => 4,
            bits => bits.div_ceil(8) + 1,
        }
    }

    fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
        const { assert_compact_supported(BITS) }

        match self.0.bit_len() {
            // 0..=0b0011_1111
            0..=6 => dest.push_byte((self.0.to::<u8>()) << 2),
            // 0..=0b0011_1111_1111_1111
            7..=14 => ((self.0.to::<u16>() << 2) | 0b01).encode_to(dest),
            // 0..=0b0011_1111_1111_1111_1111_1111_1111_1111
            15..=30 => ((self.0.to::<u32>() << 2) | 0b10).encode_to(dest),
            _ => {
                let bytes_needed = self.0.byte_len();
                assert!(
                    bytes_needed >= 4,
                    "Previous match arm matches anything less than 2^30; qed"
                );
                #[allow(clippy::cast_possible_truncation)] // bytes_needed <
                dest.push_byte(0b11 + ((bytes_needed - 4) << 2) as u8);
                dest.write(&self.0.as_le_bytes_trimmed());
            }
        }
    }
}

/// Prefix another input with a byte.
struct PrefixInput<'a, T> {
    prefix: Option<u8>,
    input:  &'a mut T,
}

impl<'a, T: 'a + Input> Input for PrefixInput<'a, T> {
    fn remaining_len(&mut self) -> Result<Option<usize>, Error> {
        let len = if let Some(len) = self.input.remaining_len()? {
            Some(len.saturating_add(self.prefix.iter().count()))
        } else {
            None
        };
        Ok(len)
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        match self.prefix.take() {
            Some(v) if !buffer.is_empty() => {
                buffer[0] = v;
                self.input.read(&mut buffer[1..])
            }
            _ => self.input.read(buffer),
        }
    }
}

const OUT_OF_RANGE: &str = "out of range Uint decoding";

impl<const BITS: usize, const LIMBS: usize> Decode for CompactUint<BITS, LIMBS> {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        const { assert_compact_supported(BITS) }

        let prefix = input.read_byte()?;
        Ok(Self(match prefix % 4 {
            0 => {
                Uint::<BITS, LIMBS>::try_from(prefix >> 2).map_err(|_| Error::from(OUT_OF_RANGE))?
            } // right shift to remove mode bits
            1 => {
                let x = u16::decode(&mut PrefixInput {
                    prefix: Some(prefix),
                    input,
                })? >> 2; // right shift to remove mode bits
                if (0b0100_0000..=0b0011_1111_1111_1111).contains(&x) {
                    x.try_into().map_err(|_| Error::from(OUT_OF_RANGE))?
                } else {
                    return Err(Error::from(OUT_OF_RANGE));
                }
            }
            2 => {
                let x = u32::decode(&mut PrefixInput {
                    prefix: Some(prefix),
                    input,
                })? >> 2; // right shift to remove mode bits
                if (0b0100_0000_0000_0000..=u32::MAX >> 2).contains(&x) {
                    x.try_into().map_err(|_| Error::from(OUT_OF_RANGE))?
                } else {
                    return Err(OUT_OF_RANGE.into());
                }
            }
            _ => match (prefix >> 2) + 4 {
                4 => {
                    let x = u32::decode(input)?;
                    if x > u32::MAX >> 2 {
                        x.try_into().map_err(|_| Error::from(OUT_OF_RANGE))?
                    } else {
                        return Err(OUT_OF_RANGE.into());
                    }
                }
                8 => {
                    let x = u64::decode(input)?;
                    if x > u64::MAX >> 8 {
                        x.try_into().map_err(|_| Error::from(OUT_OF_RANGE))?
                    } else {
                        return Err(OUT_OF_RANGE.into());
                    }
                }
                16 => {
                    let x = u128::decode(input)?;
                    if x > u128::MAX >> 8 {
                        x.try_into().map_err(|_| Error::from(OUT_OF_RANGE))?
                    } else {
                        return Err(OUT_OF_RANGE.into());
                    }
                }
                bytes => {
                    let le_byte_slice = (0..bytes)
                        .map(|_| input.read_byte())
                        .collect::<Result<Vec<_>, _>>()?;
                    let x = Uint::<BITS, LIMBS>::try_from_le_slice(&le_byte_slice)
                        .ok_or_else(|| Error::from("value is larger than fits the Uint"))?;
                    if le_byte_slice.last() != Some(&0) {
                        x
                    } else {
                        return Err(OUT_OF_RANGE.into());
                    }
                }
            },
        }))
    }
}

const fn assert_compact_supported(bits: usize) {
    assert!(
        bits <= COMPACT_BITS_LIMIT,
        "compact encoding is supported only for 0-(2**536-1) values"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Uint, aliases::U256, const_for, nlimbs};
    use proptest::proptest;

    #[test]
    fn test_scale() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            proptest!(|(value: Uint<BITS, LIMBS>)| {
                let serialized = Encode::encode(&value);
                let deserialized = <Uint::<BITS, LIMBS> as Decode>::decode(&mut serialized.as_slice()).unwrap();
                assert_eq!(value, deserialized);
            });
        });
    }

    #[test]
    fn test_scale_compact() {
        const_for!(BITS in [1, 2, 3, 7, 8, 9, 15, 16, 17, 29, 30, 31, 32, 33, 63, 64, 65, 127, 128, 129, 256, 384, 512, 535] {
            const LIMBS: usize = nlimbs(BITS);
            proptest!(|(value: Uint<BITS, LIMBS>)| {
                // value.serialize_compact().deserialize_compact() == value
                let serialized_compact = CompactRefUint(&value).encode();
                let deserialized_compact = CompactUint::decode(&mut serialized_compact.as_slice()).unwrap();
                assert_eq!(value, deserialized_compact.0);

                // Only for 0-(2**128-1) values.
                // Check that our compact implementation is the same as parity-scale-codec's.
                // value.serialize_compact_parity() == value.serialize_compact()
                let value_u128: Result<u128, _> = value.try_into();
                if let Ok(value_u128) = value_u128 {
                    #[allow(clippy::cast_possible_truncation)] // value < 2**BITS
                    match BITS {
                        0..=8 => assert_eq!(serialized_compact, Compact(value_u128 as u8).encode()),
                        9..=16 => assert_eq!(serialized_compact, Compact(value_u128 as u16).encode()),
                        17..=32 => assert_eq!(serialized_compact, Compact(value_u128 as u32).encode()),
                        33..=64 => assert_eq!(serialized_compact, Compact(value_u128 as u64).encode()),
                        65..=128 => assert_eq!(serialized_compact, Compact(value_u128).encode()),
                        _ => {}
                    }
                }
            });
        });
    }

    #[test]
    fn test_scale_compact_size_hint() {
        const_for!(BITS in [31, 64, 65, 128, 256, 384, 512, 535] {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            for bits in 0..=BITS {
                let value = U::MAX >> (BITS - bits);
                let mut encoded = Vec::new();
                CompactRefUint(&value).encode_to(&mut encoded);
                assert_eq!(CompactRefUint(&value).size_hint(), encoded.len(), "{value}");
            }
        });
    }

    fn decode_compact(bytes: &[u8]) -> Result<U256, Error> {
        CompactUint::<256, 4>::decode(&mut &*bytes).map(|c| c.0)
    }

    #[test]
    fn test_scale_compact_rejects_non_canonical_mode1() {
        // Mode 1 encoding of 63, which fits in mode 0.
        assert!(decode_compact(&[0xfd, 0x00]).is_err());
    }

    #[test]
    fn test_scale_compact_rejects_non_canonical_mode2() {
        // Mode 2 encoding of 2**14 - 1, which fits in mode 1.
        assert!(decode_compact(&[0xfe, 0xff, 0x00, 0x00]).is_err());
    }

    #[test]
    fn test_scale_compact_rejects_non_canonical_big_integer() {
        // Big-integer encodings with a zero most-significant byte.
        assert!(decode_compact(&[0x07, 0x01, 0x00, 0x00, 0x00, 0x00]).is_err());
        assert!(decode_compact(&[&[0x73, 1][..], &[0; 31]].concat()).is_err());
    }

    #[test]
    fn test_scale_compact_boundaries() {
        let mut values = vec![U256::ZERO, U256::from(1)];
        for bits in [6, 14, 30] {
            values.push(U256::from(1u64 << bits) - U256::from(1));
            values.push(U256::from(1u64 << bits));
        }
        for bits in (32..256).step_by(8) {
            values.push((U256::from(1) << bits) - U256::from(1));
            values.push(U256::from(1) << bits);
        }

        for value in values {
            let canonical = CompactRefUint(&value).encode();
            assert_eq!(decode_compact(&canonical).unwrap(), value);

            // Every other encoding of `value`, one per mode and big-integer
            // byte count.
            let le = value.to_le_bytes::<32>();
            let mut encodings = Vec::new();
            if value < U256::from(1u64 << 6) {
                encodings.push(vec![le[0] << 2]);
            }
            if value < U256::from(1u64 << 14) {
                let x = u16::from_le_bytes([le[0], le[1]]);
                encodings.push(((x << 2) | 0b01).to_le_bytes().to_vec());
            }
            if value < U256::from(1u64 << 30) {
                let x = u32::from_le_bytes([le[0], le[1], le[2], le[3]]);
                encodings.push(((x << 2) | 0b10).to_le_bytes().to_vec());
            }
            for n in 4..=32usize {
                if le[n..].iter().all(|&b| b == 0) {
                    encodings.push([&[(((n - 4) << 2) | 0b11) as u8][..], &le[..n]].concat());
                }
            }
            for encoding in encodings {
                assert_eq!(
                    decode_compact(&encoding).is_ok(),
                    encoding == canonical,
                    "value {value}, encoding {encoding:02x?}"
                );
            }
        }
    }

    #[test]
    fn test_scale_compact_derive() {
        #[allow(clippy::semicolon_if_nothing_returned)] // False positive from macro expansion
        #[derive(Debug, PartialEq, Encode, Decode)]
        struct Data {
            #[codec(compact)]
            value: U256,
        }

        let data = Data { value: U256::MAX };
        let serialized = data.encode();
        let deserialized = Data::decode(&mut serialized.as_slice()).unwrap();

        assert_eq!(data, deserialized);
    }
}
