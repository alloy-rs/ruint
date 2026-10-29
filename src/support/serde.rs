//! Support for the [`serde`](https://crates.io/crates/serde) crate.

#![cfg(feature = "serde")]
#![cfg_attr(docsrs, doc(cfg(feature = "serde")))]

use crate::{Bits, Int, Uint, fmt::StackString, nbytes};
use core::{fmt, str};
use serde_core::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, Unexpected, Visitor},
};

/// Canonical serialization for all human-readable instances of `Uint<0, 0>`,
/// and minimal human-readable `Uint<BITS, LIMBS>::ZERO` for any bit size.
const ZERO_STR: &str = "0x0";

impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    fn serialize_human<const FULL: bool, S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Ideally `SIZE` is `2 + Self::BYTES * 2`.
        if BITS <= 8 {
            self.serialize_human_sized::<FULL, 8, _>(s)
        } else {
            self.serialize_human_sized::<FULL, BITS, _>(s)
        }
    }

    fn serialize_human_sized<const FULL: bool, const SIZE: usize, S: Serializer>(
        &self,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let mut buffer = StackString::<SIZE>::new();
        self.write_hex::<FULL>(&mut buffer);
        s.serialize_str(buffer.as_str())
    }

    fn serialize_binary<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&self.to_be_bytes_vec())
    }

    fn write_hex<const FULL: bool>(&self, s: &mut (impl fmt::Write + ?Sized)) {
        if BITS == 0 {
            s.write_str(ZERO_STR)
        } else if FULL {
            write!(s, "{:#0w$x}", *self, w = 2 + Self::BYTES * 2)
        } else {
            write!(s, "{:#x}", *self)
        }
        .unwrap();
    }
}

/// Serialize a [`Uint`] value.
///
/// For human readable formats a `0x` prefixed lower case hex string is used.
/// For binary formats a byte array is used. Leading zeros are included.
impl<const BITS: usize, const LIMBS: usize> Serialize for Uint<BITS, LIMBS> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            self.serialize_human::<false, _>(serializer)
        } else {
            self.serialize_binary(serializer)
        }
    }
}

/// Deserialize human readable hex strings or byte arrays into [`Uint`].
///
/// Hex strings can be upper/lower/mixed case, have an optional `0x` prefix, and
/// can be any length. They are interpreted big-endian.
impl<'de, const BITS: usize, const LIMBS: usize> Deserialize<'de> for Uint<BITS, LIMBS> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(HrVisitor)
        } else {
            deserializer.deserialize_bytes(ByteVisitor)
        }
    }
}

impl<const BITS: usize, const LIMBS: usize> Serialize for Bits<BITS, LIMBS> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            self.as_uint().serialize_human::<true, _>(serializer)
        } else {
            self.as_uint().serialize_binary(serializer)
        }
    }
}

impl<'de, const BITS: usize, const LIMBS: usize> Deserialize<'de> for Bits<BITS, LIMBS> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Uint::deserialize(deserializer).map(Self::from)
    }
}

/// Serialize an [`Int`] value.
///
/// For human readable formats a decimal string is used. For binary formats the
/// two's complement representation is serialized like a [`Uint`].
impl<const BITS: usize, const LIMBS: usize> Serialize for Int<BITS, LIMBS> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.collect_str(self)
        } else {
            self.cast_unsigned().serialize_binary(serializer)
        }
    }
}

/// Deserialize human readable strings or numbers, or byte arrays into [`Int`].
///
/// Strings are parsed with [`FromStr`](str::FromStr): an optional sign followed
/// by decimal digits or `0x`, `0o` or `0b` prefixed digits. Byte arrays hold
/// the two's complement representation, as for [`Uint`].
impl<'de, const BITS: usize, const LIMBS: usize> Deserialize<'de> for Int<BITS, LIMBS> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(IntHrVisitor)
        } else {
            Uint::deserialize(deserializer).map(Uint::cast_signed)
        }
    }
}

/// Serde Visitor for human readable formats.
///
/// Accepts either a primitive number, a decimal or a hexadecimal string.
struct HrVisitor<const BITS: usize, const LIMBS: usize>;

impl<const BITS: usize, const LIMBS: usize> Visitor<'_> for HrVisitor<BITS, LIMBS> {
    type Value = Uint<BITS, LIMBS>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a {} byte hex string", nbytes(BITS))
    }

    fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
        Uint::try_from(v).map_err(|_| Error::invalid_value(Unexpected::Unsigned(v), &self))
    }

    fn visit_u128<E: Error>(self, v: u128) -> Result<Self::Value, E> {
        // `Unexpected::Unsigned` cannot contain a `u128`
        Uint::try_from(v).map_err(Error::custom)
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
        // Shortcut for common case
        if value == ZERO_STR {
            return Ok(Uint::<BITS, LIMBS>::ZERO);
        }
        // `ZERO_STR` is the only valid serialization of `Uint<0, 0>`, so if we
        // have not shortcut, we are in an error case
        if BITS == 0 {
            return Err(Error::invalid_value(Unexpected::Str(value), &self));
        }

        value
            .parse()
            .map_err(|_| Error::invalid_value(Unexpected::Str(value), &self))
    }
}

/// Serde Visitor for human readable [`Int`] formats.
///
/// Accepts either a primitive number or a signed number string.
struct IntHrVisitor<const BITS: usize, const LIMBS: usize>;

impl<const BITS: usize, const LIMBS: usize> Visitor<'_> for IntHrVisitor<BITS, LIMBS> {
    type Value = Int<BITS, LIMBS>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a {BITS} bit signed integer")
    }

    fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
        Int::try_from(v).map_err(|_| Error::invalid_value(Unexpected::Unsigned(v), &self))
    }

    fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> {
        Int::try_from(v).map_err(|_| Error::invalid_value(Unexpected::Signed(v), &self))
    }

    fn visit_u128<E: Error>(self, v: u128) -> Result<Self::Value, E> {
        // `Unexpected::Unsigned` cannot contain a `u128`
        Int::try_from(v).map_err(Error::custom)
    }

    fn visit_i128<E: Error>(self, v: i128) -> Result<Self::Value, E> {
        // `Unexpected::Signed` cannot contain an `i128`
        Int::try_from(v).map_err(Error::custom)
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
        value
            .parse()
            .map_err(|_| Error::invalid_value(Unexpected::Str(value), &self))
    }
}

/// Serde Visitor for non-human readable formats
struct ByteVisitor<const BITS: usize, const LIMBS: usize>;

impl<const BITS: usize, const LIMBS: usize> Visitor<'_> for ByteVisitor<BITS, LIMBS> {
    type Value = Uint<BITS, LIMBS>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{BITS} bits of binary data in big endian order")
    }

    fn visit_bytes<E: Error>(self, value: &[u8]) -> Result<Self::Value, E> {
        if value.len() != nbytes(BITS) {
            return Err(Error::invalid_length(value.len(), &self));
        }
        Uint::try_from_be_slice(value).ok_or_else(|| {
            Error::invalid_value(
                Unexpected::Other(&format!("too large for Uint<{BITS}>")),
                &self,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{aliases::I256, const_for, nlimbs};
    use proptest::proptest;

    #[allow(unused_imports)]
    use alloc::vec::Vec;

    #[test]
    fn test_serde_human_readable() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            proptest!(|(value: Uint<BITS, LIMBS>)| {
                let s = format!("U{BITS} => {value}");
                let serialized = serde_json::to_string(&value).expect(&s);
                let deserialized = serde_json::from_str::<Uint<BITS, LIMBS>>(&serialized).expect(&s);
                assert_eq!(value, deserialized);
            });
            proptest!(|(value: Bits<BITS, LIMBS>)| {
                let serialized = serde_json::to_string(&value).unwrap();
                let deserialized = serde_json::from_str::<Bits<BITS, LIMBS>>(&serialized).unwrap();
                assert_eq!(value, deserialized);
            });
        });
    }

    #[test]
    fn test_human_readable_de() {
        let jason = r#"[
            1,
            "0x1",
            "0o1",
            "0b1"
        ]"#;
        let numbers: Vec<Uint<1, 1>> = serde_json::from_str(jason).unwrap();
        uint! {
            assert_eq!(numbers, vec![1_U1, 1_U1, 1_U1, 1_U1]);
        }

        let jason = r#"[
            "",
            "0x",
            "0o",
            "0b"
        ]"#;
        let numbers: Vec<Uint<1, 1>> = serde_json::from_str(jason).unwrap();
        uint! {
            assert_eq!(numbers, vec![0_U1, 0_U1, 0_U1, 0_U1]);
        }
    }

    #[test]
    fn test_serde_machine_readable() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            proptest!(|(value: Uint<BITS, LIMBS>)| {
                let serialized = bincode::serialize(&value).unwrap();
                let deserialized = bincode::deserialize::<Uint<BITS, LIMBS>>(&serialized[..]).unwrap();
                assert_eq!(value, deserialized);
            });
            proptest!(|(value: Bits<BITS, LIMBS>)| {
                let serialized = bincode::serialize(&value).unwrap();
                let deserialized = bincode::deserialize::<Bits<BITS, LIMBS>>(&serialized[..]).unwrap();
                assert_eq!(value, deserialized);
            });
        });
    }

    #[test]
    fn test_serde_invalid_size_error() {
        // Test that if we add a character to a value that is already the max
        // length for the given number of bits, we get an error.
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            let value = Uint::<BITS, LIMBS>::MAX;
            let mut serialized = serde_json::to_string(&value).unwrap();

            // ensure format of serialized value is correct ("0x...")
            assert_eq!(&serialized[..3], "\"0x");
            // last character should be a quote
            assert_eq!(&serialized[serialized.len() - 1..], "\"");

            // strip the last character, add a zero, and finish with a quote
            serialized.pop();
            serialized.push('0');
            serialized.push('"');
            let deserialized = serde_json::from_str::<Uint<BITS, LIMBS>>(&serialized);
            assert!(deserialized.is_err(), "U{BITS} {serialized} => {deserialized:?}");
        });
    }

    const INT_TEST_VALS: [&str; 5] = [
        "12345681238128738123",
        "-1239373781294184527124318238",
        "99999999999999999999999999999999999999999999999999999999999",
        "57896044618658097711785492504343953926634992332820282019728792003956564819967",
        "-57896044618658097711785492504343953926634992332820282019728792003956564819968",
    ];

    #[test]
    fn test_int_serde_json() {
        for val in INT_TEST_VALS {
            let signed = val.parse::<I256>().unwrap();
            let serialized = serde_json::to_string(&signed).unwrap();
            assert_eq!(serialized, format!("\"{val}\""));
            assert_eq!(serde_json::from_str::<I256>(&serialized).unwrap(), signed);
        }
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            proptest!(|(value: Int<BITS, LIMBS>)| {
                let serialized = serde_json::to_string(&value).unwrap();
                assert_eq!(serialized, format!("\"{value}\""));
                let deserialized = serde_json::from_str::<Int<BITS, LIMBS>>(&serialized).unwrap();
                assert_eq!(value, deserialized);
            });
        });
    }

    #[test]
    fn test_int_human_readable_de() {
        let jason = r#"[
            1,
            -1,
            -9223372036854775808,
            18446744073709551615,
            "-0x1",
            "+0o1",
            "0b1",
            "-57896044618658097711785492504343953926634992332820282019728792003956564819968"
        ]"#;
        let numbers: Vec<I256> = serde_json::from_str(jason).unwrap();
        uint! {
            assert_eq!(numbers, vec![
                1_I256,
                -1_I256,
                I256::try_from(i64::MIN).unwrap(),
                I256::try_from(u64::MAX).unwrap(),
                -1_I256,
                1_I256,
                1_I256,
                I256::MIN,
            ]);
        }

        for jason in [
            "\"57896044618658097711785492504343953926634992332820282019728792003956564819968\"",
            "\"-57896044618658097711785492504343953926634992332820282019728792003956564819969\"",
            "\"0x8000000000000000000000000000000000000000000000000000000000000000\"",
            "\"--1\"",
            "1.5",
            "[]",
        ] {
            assert!(serde_json::from_str::<I256>(jason).is_err(), "{jason}");
        }
        assert!(serde_json::from_str::<Int<8, 1>>("128").is_err());
        assert!(serde_json::from_str::<Int<8, 1>>("-129").is_err());
        assert_eq!(serde_json::from_str::<Int<8, 1>>("-128").unwrap(), Int::MIN);
    }

    #[test]
    fn test_int_serde_machine_readable() {
        for val in INT_TEST_VALS {
            let signed = val.parse::<I256>().unwrap();
            let serialized = bincode::serialize(&signed).unwrap();
            assert!(serialized.len() <= 64);
            assert_eq!(bincode::deserialize::<I256>(&serialized).unwrap(), signed);
        }
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            proptest!(|(value: Int<BITS, LIMBS>)| {
                let serialized = bincode::serialize(&value).unwrap();
                assert_eq!(serialized, bincode::serialize(&value.cast_unsigned()).unwrap());
                let deserialized = bincode::deserialize::<Int<BITS, LIMBS>>(&serialized[..]).unwrap();
                assert_eq!(value, deserialized);
            });
        });
    }
}
