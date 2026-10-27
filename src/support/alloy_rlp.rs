//! Support for the [`alloy-rlp`](https://crates.io/crates/alloy-rlp) crate.

#![cfg(feature = "alloy-rlp")]
#![cfg_attr(docsrs, doc(cfg(feature = "alloy-rlp")))]

use crate::Uint;
use alloy_rlp::{
    BufMut, Decodable, EMPTY_STRING_CODE, Encodable, Error, Header, MaxEncodedLen,
    MaxEncodedLenAssoc, length_of_length,
};

/// Allows a [`Uint`] to be serialized as RLP.
///
/// See <https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/>
impl<const BITS: usize, const LIMBS: usize> Encodable for Uint<BITS, LIMBS> {
    #[inline]
    fn length(&self) -> usize {
        let bits = self.bit_len();
        if bits <= 7 {
            1
        } else {
            let bytes = bits.div_ceil(8);
            bytes + length_of_length(bytes)
        }
    }

    #[inline]
    fn encode(&self, out: &mut dyn BufMut) {
        // fast paths, avoiding allocation due to `to_be_bytes_vec`
        match LIMBS {
            0 => return out.put_u8(EMPTY_STRING_CODE),
            1 => return self.limbs[0].encode(out),
            #[allow(clippy::cast_lossless)]
            2 => return (self.limbs[0] as u128 | ((self.limbs[1] as u128) << 64)).encode(out),
            _ => {}
        }

        let bits = self.bit_len();
        if bits <= 7 {
            #[allow(clippy::cast_possible_truncation)] // self < 128
            return out.put_u8(if bits == 0 {
                EMPTY_STRING_CODE
            } else {
                self.limbs[0] as u8
            });
        }
        let len = bits.div_ceil(8);
        if Self::BYTES <= 55 {
            // Write the header and the big-endian payload with one `put_slice`.
            let mut buf = [0u8; 64];
            for (i, limb) in self.limbs.iter().enumerate() {
                let at = 64 - 8 * (i + 1);
                buf[at..at + 8].copy_from_slice(&limb.to_be_bytes());
            }
            let start = 63 - len;
            #[allow(clippy::cast_possible_truncation)] // len < 56
            let header = EMPTY_STRING_CODE + len as u8;
            buf[start] = header;
            return out.put_slice(&buf[start..]);
        }

        // avoid heap allocation in `to_be_bytes_vec`
        // SAFETY: we don't re-use `copy`
        #[cfg(target_endian = "little")]
        let mut copy = *self;
        #[cfg(target_endian = "little")]
        let bytes = unsafe { copy.as_le_slice_mut() };
        #[cfg(target_endian = "little")]
        bytes.reverse();

        #[cfg(target_endian = "big")]
        let bytes = self.to_be_bytes_vec();

        bytes[Self::BYTES - len..].encode(out);
    }
}

/// Allows a [`Uint`] to be deserialized from RLP.
///
/// See <https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/>
impl<const BITS: usize, const LIMBS: usize> Decodable for Uint<BITS, LIMBS> {
    #[inline]
    fn decode(buf: &mut &[u8]) -> Result<Self, Error> {
        let bytes = Header::decode_bytes(buf, false)?;

        // The RLP spec states that deserialized positive integers with leading
        // zeroes get treated as invalid.
        //
        // See:
        // https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/
        //
        // To check this, we only need to check if the first byte is zero to
        // make sure there are no leading zeros
        if !bytes.is_empty() && bytes[0] == 0 {
            return Err(Error::LeadingZero);
        }

        Self::try_from_be_slice(bytes).ok_or(Error::Overflow)
    }
}

#[cfg(feature = "generic_const_exprs")]
unsafe impl<const BITS: usize, const LIMBS: usize>
    MaxEncodedLen<{ Self::BYTES + length_of_length(Self::BYTES) }> for Uint<BITS, LIMBS>
{
}

#[cfg(not(feature = "generic_const_exprs"))]
const _: () = {
    crate::const_for!(BITS in [0, 1, 2, 8, 16, 32, 64, 128, 160, 192, 256, 384, 512, 4096] {
        const LIMBS: usize = crate::nlimbs(BITS);
        const BYTES: usize = Uint::<BITS, LIMBS>::BYTES;
        unsafe impl MaxEncodedLen<{ BYTES + length_of_length(BYTES) }> for Uint<BITS, LIMBS> {}
    });
};

unsafe impl<const BITS: usize, const LIMBS: usize> MaxEncodedLenAssoc for Uint<BITS, LIMBS> {
    const LEN: usize = Self::BYTES + length_of_length(Self::BYTES);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        aliases::{U0, U256},
        const_for, nlimbs,
    };
    use hex_literal::hex;
    use proptest::proptest;

    fn encode<T: Encodable>(value: T) -> Vec<u8> {
        let mut buf = vec![];
        value.encode(&mut buf);
        buf
    }

    #[test]
    fn test_rlp() {
        // See <https://github.com/paritytech/parity-common/blob/436cb0827f0e3238ccb80d7d453f756d126c0615/rlp/tests/tests.rs#L214>
        assert_eq!(encode(U0::from(0))[..], hex!("80"));
        assert_eq!(encode(U256::from(0))[..], hex!("80"));
        assert_eq!(encode(U256::from(15))[..], hex!("0f"));
        assert_eq!(encode(U256::from(1024))[..], hex!("820400"));
        assert_eq!(encode(U256::from(0x1234_5678))[..], hex!("8412345678"));
    }

    #[test]
    fn test_roundtrip() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            proptest!(|(value: Uint<BITS, LIMBS>)| {
                let serialized = encode(value);

                #[cfg(feature = "rlp")]
                {
                    use rlp::Encodable as _;
                    let serialized_rlp = value.rlp_bytes();
                    assert_eq!(serialized, serialized_rlp.freeze()[..]);
                }

                assert_eq!(serialized.len(), value.length());
                let mut reader = &serialized[..];
                let deserialized = Uint::decode(&mut reader).unwrap();
                assert_eq!(reader.len(), 0);
                assert_eq!(value, deserialized);
            });
        });
    }

    #[test]
    fn test_roundtrip_55_byte_boundary() {
        const_for!(BITS in [432, 433, 440, 441, 448] {
            const LIMBS: usize = nlimbs(BITS);
            for shift in [0_usize, 1, 7, 8, 63, 64] {
                let value = Uint::<BITS, LIMBS>::MAX >> shift;
                let serialized = encode(value);

                #[cfg(feature = "rlp")]
                {
                    use rlp::Encodable as _;
                    assert_eq!(serialized, value.rlp_bytes().freeze()[..]);
                }

                assert_eq!(serialized.len(), value.length());
                assert_eq!(Uint::<BITS, LIMBS>::decode(&mut &serialized[..]), Ok(value));
            }
        });
    }

    #[test]
    fn test_invalid_uints() {
        // these are non-canonical because they have leading zeros
        assert_eq!(
            U256::decode(&mut &hex!("820000")[..]),
            Err(Error::LeadingZero)
        );
        // 00 is not a valid uint
        // See https://github.com/ethereum/go-ethereum/blob/cd2953567268777507b1ec29269315324fb5aa9c/rlp/decode_test.go#L118
        assert_eq!(U256::decode(&mut &hex!("00")[..]), Err(Error::LeadingZero));
        // these are non-canonical because they can fit in a single byte, i.e.
        // 0x7f, 0x33
        assert_eq!(
            U256::decode(&mut &hex!("8100")[..]),
            Err(Error::NonCanonicalSingleByte)
        );
        assert_eq!(
            U256::decode(&mut &hex!("817f")[..]),
            Err(Error::NonCanonicalSingleByte)
        );
        assert_eq!(
            U256::decode(&mut &hex!("8133")[..]),
            Err(Error::NonCanonicalSingleByte)
        );
    }
}
