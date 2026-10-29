//! Support for the [`postgres`](https://crates.io/crates/postgres) crate.

#![cfg(feature = "postgres")]
#![cfg_attr(docsrs, doc(cfg(feature = "postgres")))]

use crate::{
    Int, Sign, Uint,
    utils::{rem_up, trim_end_vec},
};
use bytes::{BufMut, BytesMut};
use core::{
    error::Error,
    str::{FromStr, from_utf8},
};
use postgres_types::{FromSql, IsNull, ToSql, Type, WrongType, to_sql_checked};
use thiserror::Error;

type BoxedError = Box<dyn Error + Sync + Send + 'static>;

#[derive(Clone, PartialEq, Eq, Debug, Error)]
pub enum ToSqlError {
    #[error("{0}-bit integer value does not fit target type {1}")]
    Overflow(usize, Type),
}

/// Convert to Postgres types.
///
/// Compatible [Postgres data types][dt] are:
///
/// * `BOOL`, `SMALLINT`, `INTEGER`, `BIGINT` which are 1, 16, 32 and 64 bit
///   signed integers respectively.
/// * `OID` which is a 32 bit unsigned integer.
/// * `FLOAT`, `DOUBLE PRECISION` which are 32 and 64 bit floating point.
/// * `DECIMAL` and `NUMERIC`, which are variable length.
/// * `MONEY` which is a 64 bit integer with two decimals.
/// * `BYTEA`, `BIT`, `VARBIT` interpreted as a big-endian binary number.
/// * `CHAR`, `VARCHAR`, `TEXT` as `0x`-prefixed big-endian hex strings.
/// * `JSON`, `JSONB` as a hex string compatible with the Serde serialization.
///
/// Note: [`Uint`]s are never null, use [`Option<Uint>`] instead.
///
/// # Errors
///
/// Returns an error when trying to convert to a value that is too small to fit
/// the number. Note that this depends on the value, not the type, so a
/// [`Uint<256>`] can be stored in a `SMALLINT` column, as long as the values
/// are less than $2^{16}$.
///
/// # Implementation details
///
/// The Postgres binary formats are used in the wire-protocol and the
/// the `COPY BINARY` command, but they have very little documentation. You are
/// pointed to the source code, for example this is the implementation of the
/// the `NUMERIC` type serializer: [`numeric.c`][numeric].
///
/// [dt]:https://www.postgresql.org/docs/9.5/datatype.html
/// [numeric]: https://github.com/postgres/postgres/blob/05a5a1775c89f6beb326725282e7eea1373cbec8/src/backend/utils/adt/numeric.c#L1082
impl<const BITS: usize, const LIMBS: usize> ToSql for Uint<BITS, LIMBS> {
    fn accepts(ty: &Type) -> bool {
        matches!(*ty, |Type::BOOL| Type::CHAR
            | Type::INT2
            | Type::INT4
            | Type::INT8
            | Type::OID
            | Type::FLOAT4
            | Type::FLOAT8
            | Type::MONEY
            | Type::NUMERIC
            | Type::BYTEA
            | Type::TEXT
            | Type::VARCHAR
            | Type::JSON
            | Type::JSONB
            | Type::BIT
            | Type::VARBIT)
    }

    // See <https://github.com/sfackler/rust-postgres/blob/38da7fa8fe0067f47b60c147ccdaa214ab5f5211/postgres-protocol/src/types/mod.rs>
    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxedError> {
        match *ty {
            // Big-endian simple types
            // Note `BufMut::put_*` methods write big-endian by default.
            Type::BOOL => out.put_u8(u8::from(bool::try_from(*self)?)),
            Type::INT2 => out.put_i16(self.try_into()?),
            Type::INT4 => out.put_i32(self.try_into()?),
            Type::OID => out.put_u32(self.try_into()?),
            Type::INT8 => out.put_i64(self.try_into()?),
            Type::FLOAT4 => out.put_f32(self.into()),
            Type::FLOAT8 => out.put_f64(self.into()),
            Type::MONEY => {
                // Like i64, but with two decimals.
                out.put_i64(
                    i64::try_from(self)?
                        .checked_mul(100)
                        .ok_or_else(|| ToSqlError::Overflow(BITS, ty.clone()))?,
                );
            }

            // Binary strings
            Type::BYTEA => out.put_slice(&self.to_be_bytes_vec()),
            Type::BIT | Type::VARBIT => {
                // Bit in little-endian so the the first bit is the least
                // significant. Length must be at least one bit.
                if BITS == 0 {
                    if *ty == Type::BIT {
                        // `bit(0)` is not a valid type, but varbit can be
                        // empty.
                        return Err(Box::new(WrongType::new::<Self>(ty.clone())));
                    }
                    out.put_i32(0);
                } else {
                    // Bits are output in big-endian order, but padded at the
                    // least significant end.
                    let padding = 8 - rem_up(BITS, 8);
                    out.put_i32(Self::BITS.try_into()?);
                    let bytes = self.as_le_bytes();
                    let mut bytes = bytes.iter().rev();
                    let mut shifted = bytes.next().unwrap() << padding;
                    for byte in bytes {
                        shifted |= if padding > 0 {
                            byte >> (8 - padding)
                        } else {
                            0
                        };
                        out.put_u8(shifted);
                        shifted = byte << padding;
                    }
                    out.put_u8(shifted);
                }
            }

            // Hex strings
            Type::CHAR | Type::TEXT | Type::VARCHAR => {
                out.put_slice(format!("{self:#x}").as_bytes());
            }
            Type::JSON | Type::JSONB => {
                if *ty == Type::JSONB {
                    // Version 1 of JSONB is just plain text JSON.
                    out.put_u8(1);
                }
                out.put_slice(format!("\"{self:#x}\"").as_bytes());
            }

            // Binary coded decimal types
            // See <https://github.com/postgres/postgres/blob/05a5a1775c89f6beb326725282e7eea1373cbec8/src/backend/utils/adt/numeric.c#L253>
            Type::NUMERIC => numeric_to_sql(Sign::Positive, self, out)?,

            // Unsupported types
            _ => {
                return Err(Box::new(WrongType::new::<Self>(ty.clone())));
            }
        }
        Ok(IsNull::No)
    }

    to_sql_checked!();
}

#[derive(Clone, PartialEq, Eq, Debug, Error)]
pub enum FromSqlError {
    #[error("The value does not fit the integer type")]
    Overflow,

    #[error("Unexpected data for type {0}")]
    ParseError(Type),
}

/// Convert from Postgres types.
///
/// See [`ToSql`][Self::to_sql] for details.
impl<'a, const BITS: usize, const LIMBS: usize> FromSql<'a> for Uint<BITS, LIMBS> {
    fn accepts(ty: &Type) -> bool {
        <Self as ToSql>::accepts(ty)
    }

    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        Ok(match *ty {
            Type::BOOL => match raw {
                [0] => Self::ZERO,
                [1] => Self::try_from(1)?,
                _ => return Err(Box::new(FromSqlError::ParseError(ty.clone()))),
            },
            Type::INT2 => i16::from_be_bytes(raw.try_into()?).try_into()?,
            Type::INT4 => i32::from_be_bytes(raw.try_into()?).try_into()?,
            Type::OID => u32::from_be_bytes(raw.try_into()?).try_into()?,
            Type::INT8 => i64::from_be_bytes(raw.try_into()?).try_into()?,
            Type::FLOAT4 => f32::from_be_bytes(raw.try_into()?).try_into()?,
            Type::FLOAT8 => f64::from_be_bytes(raw.try_into()?).try_into()?,
            Type::MONEY => (i64::from_be_bytes(raw.try_into()?) / 100).try_into()?,

            // Binary strings
            Type::BYTEA => Self::try_from_be_slice(raw).ok_or(FromSqlError::Overflow)?,
            Type::BIT | Type::VARBIT => {
                // Parse header
                if raw.len() < 4 {
                    return Err(Box::new(FromSqlError::ParseError(ty.clone())));
                }
                let len: usize = i32::from_be_bytes(raw[..4].try_into()?).try_into()?;
                let raw = &raw[4..];
                if raw.len() != len.div_ceil(8) {
                    return Err(Box::new(FromSqlError::ParseError(ty.clone())));
                }

                // Shift padding to the other end
                let padding = 8 - rem_up(len, 8);
                let mut raw = raw.to_owned();
                if padding > 0 {
                    for i in (1..raw.len()).rev() {
                        raw[i] = (raw[i] >> padding) | (raw[i - 1] << (8 - padding));
                    }
                    raw[0] >>= padding;
                }
                // Construct from bits
                Self::try_from_be_slice(&raw).ok_or(FromSqlError::Overflow)?
            }

            // Hex strings
            Type::CHAR | Type::TEXT | Type::VARCHAR => Self::from_str(from_utf8(raw)?)?,

            // Hex strings
            Type::JSON | Type::JSONB => {
                let raw = if *ty == Type::JSONB {
                    let [1, rest @ ..] = raw else {
                        // Missing or unsupported version
                        return Err(Box::new(FromSqlError::ParseError(ty.clone())));
                    };
                    rest
                } else {
                    raw
                };
                let str = from_utf8(raw)?;
                // Stringified number
                let str = str
                    .strip_prefix('"')
                    .and_then(|str| str.strip_suffix('"'))
                    .unwrap_or(str);
                Self::from_str(str)?
            }

            // Numeric types
            Type::NUMERIC => match numeric_from_sql(ty, raw)? {
                (Sign::Positive, value) => value,
                (Sign::Negative, _) => return Err(Box::new(FromSqlError::ParseError(ty.clone()))),
            },

            // Unsupported types
            _ => return Err(Box::new(WrongType::new::<Self>(ty.clone()))),
        })
    }
}

/// Convert [`Int`] to Postgres types.
///
/// Compatible [Postgres data types][dt] are:
///
/// * `BOOL` for the values zero and one.
/// * `SMALLINT`, `INTEGER`, `BIGINT` which are 16, 32 and 64 bit signed
///   integers respectively.
/// * `OID` which is a 32 bit unsigned integer.
/// * `FLOAT`, `DOUBLE PRECISION` which are 32 and 64 bit floating point.
/// * `DECIMAL` and `NUMERIC`, which are variable length and signed.
/// * `MONEY` which is a 64 bit signed integer with two decimals.
/// * `BYTEA`, `BIT`, `VARBIT` holding the two's complement representation as
///   for [`Uint`].
/// * `CHAR`, `VARCHAR`, `TEXT` as `0x`-prefixed big-endian hex strings of the
///   absolute value, with a `-` prefix for negative values.
/// * `JSON`, `JSONB` as the same hex string, which the Serde deserializer
///   accepts.
///
/// Note: [`Int`]s are never null, use [`Option<Int>`] instead.
///
/// # Errors
///
/// Returns an error when the value does not fit the target type, for example
/// a negative value in an `OID` column. Like for [`Uint`], this depends on the
/// value, not the type.
///
/// [dt]:https://www.postgresql.org/docs/9.5/datatype.html
impl<const BITS: usize, const LIMBS: usize> ToSql for Int<BITS, LIMBS> {
    fn accepts(ty: &Type) -> bool {
        <Uint<BITS, LIMBS> as ToSql>::accepts(ty)
    }

    fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxedError> {
        match *ty {
            Type::BOOL => match u8::try_from(self) {
                Ok(value @ (0 | 1)) => out.put_u8(value),
                _ => return Err(Box::new(ToSqlError::Overflow(BITS, ty.clone()))),
            },
            Type::INT2 => out.put_i16(self.try_into()?),
            Type::INT4 => out.put_i32(self.try_into()?),
            Type::OID => out.put_u32(self.try_into()?),
            Type::INT8 => out.put_i64(self.try_into()?),
            Type::FLOAT4 => out.put_f32(self.into()),
            Type::FLOAT8 => out.put_f64(self.into()),
            Type::MONEY => {
                // Like i64, but with two decimals.
                out.put_i64(
                    i64::try_from(self)?
                        .checked_mul(100)
                        .ok_or_else(|| ToSqlError::Overflow(BITS, ty.clone()))?,
                );
            }

            // Binary strings
            Type::BYTEA | Type::BIT | Type::VARBIT => return self.cast_unsigned().to_sql(ty, out),

            // Signed hex strings
            Type::CHAR | Type::TEXT | Type::VARCHAR | Type::JSON | Type::JSONB => {
                let quote = if matches!(*ty, Type::JSON | Type::JSONB) {
                    "\""
                } else {
                    ""
                };
                if *ty == Type::JSONB {
                    // Version 1 of JSONB is just plain text JSON.
                    out.put_u8(1);
                }
                let sign = if self.is_negative() { "-" } else { "" };
                let abs = self.unsigned_abs();
                out.put_slice(format!("{quote}{sign}{abs:#x}{quote}").as_bytes());
            }

            Type::NUMERIC => {
                let (sign, abs) = self.into_sign_and_abs();
                numeric_to_sql(sign, &abs, out)?;
            }

            // Unsupported types
            _ => return Err(Box::new(WrongType::new::<Self>(ty.clone()))),
        }
        Ok(IsNull::No)
    }

    to_sql_checked!();
}

/// Convert [`Int`] from Postgres types.
///
/// See [`ToSql`][Self::to_sql] for details. Text and JSON values may also be
/// decimal, as parsed by [`FromStr`].
impl<'a, const BITS: usize, const LIMBS: usize> FromSql<'a> for Int<BITS, LIMBS> {
    fn accepts(ty: &Type) -> bool {
        <Self as ToSql>::accepts(ty)
    }

    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        Ok(match *ty {
            Type::BOOL => match raw {
                [0] => Self::ZERO,
                [1] => Self::try_from(1)?,
                _ => return Err(Box::new(FromSqlError::ParseError(ty.clone()))),
            },
            Type::INT2 => i16::from_be_bytes(raw.try_into()?).try_into()?,
            Type::INT4 => i32::from_be_bytes(raw.try_into()?).try_into()?,
            Type::OID => u32::from_be_bytes(raw.try_into()?).try_into()?,
            Type::INT8 => i64::from_be_bytes(raw.try_into()?).try_into()?,
            Type::FLOAT4 => f32::from_be_bytes(raw.try_into()?).try_into()?,
            Type::FLOAT8 => f64::from_be_bytes(raw.try_into()?).try_into()?,
            Type::MONEY => (i64::from_be_bytes(raw.try_into()?) / 100).try_into()?,

            // Binary strings
            Type::BYTEA | Type::BIT | Type::VARBIT => Uint::from_sql(ty, raw)?.cast_signed(),

            // Strings, JSON strings and JSON numbers
            Type::CHAR | Type::TEXT | Type::VARCHAR | Type::JSON | Type::JSONB => {
                let mut str = from_utf8(match (ty, raw) {
                    (&Type::JSONB, [1, rest @ ..]) => rest,
                    // Missing or unsupported version
                    (&Type::JSONB, _) => {
                        return Err(Box::new(FromSqlError::ParseError(ty.clone())));
                    }
                    _ => raw,
                })?;
                if matches!(*ty, Type::JSON | Type::JSONB) {
                    // Stringified number
                    str = str
                        .strip_prefix('"')
                        .and_then(|str| str.strip_suffix('"'))
                        .unwrap_or(str);
                }
                Self::from_str(str)?
            }

            Type::NUMERIC => {
                let (sign, abs) = numeric_from_sql(ty, raw)?;
                Self::checked_from_sign_and_abs(sign, abs).ok_or(FromSqlError::Overflow)?
            }

            // Unsupported types
            _ => return Err(Box::new(WrongType::new::<Self>(ty.clone()))),
        })
    }
}

/// Writes a `NUMERIC` integer.
///
/// See <https://github.com/postgres/postgres/blob/05a5a1775c89f6beb326725282e7eea1373cbec8/src/backend/utils/adt/numeric.c#L253>
fn numeric_to_sql<const BITS: usize, const LIMBS: usize>(
    sign: Sign,
    abs: &Uint<BITS, LIMBS>,
    out: &mut BytesMut,
) -> Result<(), BoxedError> {
    // Everything is done in big-endian base 1000 digits.
    const BASE: u64 = 10000;
    let mut digits: Vec<_> = abs.to_base_be(BASE).collect();
    let exponent = digits.len().saturating_sub(1).try_into()?;

    // Trailing zeros are removed.
    trim_end_vec(&mut digits, &0);

    out.put_i16(digits.len().try_into()?); // Number of digits.
    out.put_i16(exponent); // Exponent of first digit.
    // sign: 0x0000 = positive, 0x4000 = negative.
    out.put_i16(match sign {
        Sign::Positive => 0x0000,
        Sign::Negative => 0x4000,
    });
    out.put_i16(0); // dscale: Number of digits to the right of the decimal point.
    for digit in digits {
        debug_assert!(digit < BASE);
        #[allow(clippy::cast_possible_truncation)] // 10000 < i16::MAX
        out.put_i16(digit as i16);
    }
    Ok(())
}

/// Reads a `NUMERIC` integer as its sign and absolute value.
fn numeric_from_sql<const BITS: usize, const LIMBS: usize>(
    ty: &Type,
    raw: &[u8],
) -> Result<(Sign, Uint<BITS, LIMBS>), BoxedError> {
    // Parse header
    if raw.len() < 8 {
        return Err(Box::new(FromSqlError::ParseError(ty.clone())));
    }
    let digits = i32::from(i16::from_be_bytes(raw[0..2].try_into()?));
    let exponent = i32::from(i16::from_be_bytes(raw[2..4].try_into()?));
    let sign = match i16::from_be_bytes(raw[4..6].try_into()?) {
        0x0000 => Sign::Positive,
        0x4000 => Sign::Negative,
        // NaN and infinities
        _ => return Err(Box::new(FromSqlError::ParseError(ty.clone()))),
    };
    let dscale = i16::from_be_bytes(raw[6..8].try_into()?);
    let raw = &raw[8..];
    #[allow(clippy::cast_sign_loss)] // Signs are checked
    if digits < 0
        || exponent < 0
        || dscale != 0
        || digits > exponent + 1
        || raw.len() != digits as usize * 2
    {
        return Err(Box::new(FromSqlError::ParseError(ty.clone())));
    }
    let mut error = false;
    let iter = raw.as_chunks::<2>().0.iter().filter_map(|&raw| {
        if error {
            return None;
        }
        let digit = i16::from_be_bytes(raw);
        if !(0..10000).contains(&digit) {
            error = true;
            return None;
        }
        #[allow(clippy::cast_sign_loss)] // Signs are checked
        Some(digit as u64)
    });
    #[allow(clippy::cast_sign_loss)]
    // Expression can not be negative due to checks above
    let iter = iter.chain(std::iter::repeat_n(0, (exponent + 1 - digits) as usize));

    let value = Uint::from_base_be(10000, iter)?;
    if error {
        return Err(Box::new(FromSqlError::ParseError(ty.clone())));
    }
    Ok((sign, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{I8, I256},
        const_for, nbytes, nlimbs,
    };
    use approx::assert_ulps_eq;
    use hex_literal::hex;
    use postgres::{Client, NoTls};
    use proptest::{proptest, test_runner::Config as ProptestConfig};
    use std::{io::Read, sync::Mutex};

    #[test]
    fn test_basic() {
        #[allow(clippy::unreadable_literal)]
        const N: Uint<256, 4> = Uint::from_limbs([
            0xa8ec92344438aaf4_u64,
            0x9819ebdbd1faaab1_u64,
            0x573b1a7064c19c1a_u64,
            0xc85ef7d79691fe79_u64,
        ]);
        #[allow(clippy::needless_pass_by_value)]
        fn bytes(ty: Type) -> Vec<u8> {
            let mut out = BytesMut::new();
            N.to_sql(&ty, &mut out).unwrap();
            out.to_vec()
        }
        assert_eq!(bytes(Type::FLOAT4), hex!("7f800000")); // +inf
        assert_eq!(bytes(Type::FLOAT8), hex!("4fe90bdefaf2d240"));
        assert_eq!(bytes(Type::NUMERIC), hex!("0014001300000000000902760e3620f115a21c3b029709bc11e60b3e10d10d6900d123400def1c45091a147900f012f4"));
        assert_eq!(
            bytes(Type::BYTEA),
            hex!("c85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4")
        );
        assert_eq!(
            bytes(Type::BIT),
            hex!("00000100c85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4")
        );
        assert_eq!(
            bytes(Type::VARBIT),
            hex!("00000100c85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4")
        );
        assert_eq!(bytes(Type::CHAR), hex!("307863383565663764373936393166653739353733623161373036346331396331613938313965626462643166616161623161386563393233343434333861616634"));
        assert_eq!(bytes(Type::TEXT), hex!("307863383565663764373936393166653739353733623161373036346331396331613938313965626462643166616161623161386563393233343434333861616634"));
        assert_eq!(bytes(Type::VARCHAR), hex!("307863383565663764373936393166653739353733623161373036346331396331613938313965626462643166616161623161386563393233343434333861616634"));
    }

    #[test]
    fn test_roundtrip() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(value: U)| {
                let mut serialized = BytesMut::new();

                if f32::from(value).is_finite() {
                    serialized.clear();
                    if value.to_sql(&Type::FLOAT4, &mut serialized).is_ok() {
                        // println!("testing {:?} {}", value, Type::FLOAT4);
                        let deserialized = U::from_sql(&Type::FLOAT4, &serialized).unwrap();
                        assert_ulps_eq!(f32::from(value), f32::from(deserialized), max_ulps = 4);
                    }
                }
                if f64::from(value).is_finite() {
                    serialized.clear();
                    if value.to_sql(&Type::FLOAT8, &mut serialized).is_ok() {
                        // println!("testing {:?} {}", value, Type::FLOAT8);
                        let deserialized = U::from_sql(&Type::FLOAT8, &serialized).unwrap();
                        assert_ulps_eq!(f64::from(value), f64::from(deserialized), max_ulps = 4);
                    }
                }
                for ty in &[/*Type::BOOL, Type::INT2, Type::INT4, Type::INT8, Type::OID, Type::MONEY, Type::BYTEA, Type::CHAR, Type::TEXT, Type::VARCHAR, Type::JSON, Type::JSONB, Type::NUMERIC,*/ Type::BIT, Type::VARBIT] {
                    serialized.clear();
                    if value.to_sql(ty, &mut serialized).is_ok() {
                        // println!("testing {:?} {}", value, ty);
                        let deserialized = U::from_sql(ty, &serialized).unwrap();
                        assert_eq!(deserialized, value);
                    }
                }
            });
        });
    }

    // Query the binary encoding of an SQL expression
    fn get_binary(client: &mut Client, expr: &str) -> Vec<u8> {
        let query = format!("COPY (SELECT {expr}) TO STDOUT WITH BINARY;");

        // See <https://www.postgresql.org/docs/current/sql-copy.html>
        let mut reader = client.copy_out(&query).unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();

        // Parse header
        let buf = {
            const HEADER: &[u8] = b"PGCOPY\n\xff\r\n\0";
            assert_eq!(&buf[..11], HEADER);
            &buf[11 + 4..]
        };

        // Skip extension headers (must be zero length)
        assert_eq!(&buf[..4], &0_u32.to_be_bytes());
        let buf = &buf[4..];

        // Tuple field count must be one
        assert_eq!(&buf[..2], &1_i16.to_be_bytes());
        let buf = &buf[2..];

        // Field length
        let len = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        let buf = &buf[4..];

        // Field data
        let data = &buf[..len];
        let buf = &buf[len..];

        // Trailer must be -1_i16
        assert_eq!(&buf, &(-1_i16).to_be_bytes());

        data.to_owned()
    }

    fn test_to<const BITS: usize, const LIMBS: usize>(
        client: &Mutex<Client>,
        value: Uint<BITS, LIMBS>,
        ty: &Type,
    ) {
        println!("testing {value:?} {ty}");

        // Encode value locally
        let mut serialized = BytesMut::new();
        let result = value.to_sql(ty, &mut serialized);
        if result.is_err() {
            // Skip values that are out of range for the type
            return;
        }
        // Skip floating point infinities
        if ty == &Type::FLOAT4 && f32::from(value).is_infinite() {
            return;
        }
        if ty == &Type::FLOAT8 && f64::from(value).is_infinite() {
            return;
        }

        // Fetch ground truth value from Postgres
        let expr = match *ty {
            Type::BIT => format!(
                "B'{value:b}'::bit({bits})",
                value = value,
                bits = if BITS == 0 { 1 } else { BITS },
            ),
            Type::VARBIT => format!("B'{value:b}'::varbit"),
            Type::BYTEA => format!("'\\x{value:x}'::bytea"),
            Type::CHAR => format!("'{value:#x}'::char({})", 2 + 2 * nbytes(BITS)),
            Type::TEXT | Type::VARCHAR => format!("'{value:#x}'::{}", ty.name()),
            Type::JSON | Type::JSONB => format!("'\"{value:#x}\"'::{}", ty.name()),
            _ => format!("{value}::{}", ty.name()),
        };
        let ground_truth = {
            let mut client = client.lock().unwrap();
            get_binary(&mut client, &expr)
        };

        // Compare with ground truth, for float we allow tiny rounding error
        if ty == &Type::FLOAT4 {
            let serialized = f32::from_be_bytes(serialized.as_ref().try_into().unwrap());
            let ground_truth = f32::from_be_bytes(ground_truth.try_into().unwrap());
            assert_ulps_eq!(serialized, ground_truth, max_ulps = 4);
        } else if ty == &Type::FLOAT8 {
            let serialized = f64::from_be_bytes(serialized.as_ref().try_into().unwrap());
            let ground_truth = f64::from_be_bytes(ground_truth.try_into().unwrap());
            assert_ulps_eq!(serialized, ground_truth, max_ulps = 4);
        } else {
            // Check that the value is exactly the same as the ground truth
            assert_eq!(serialized, ground_truth);
        }
    }

    // This test requires a live postgresql server.
    // To start a server, run:
    //
    //     docker run -it --rm -e POSTGRES_PASSWORD=postgres -p 5432:5432
    // postgres
    //
    // Then run the test using:
    //
    //    PROPTEST_CASES=1000 cargo test --all-features -- --include-ignored
    // --nocapture postgres
    //
    #[test]
    #[ignore = "requires a live postgresql server"]
    fn test_postgres() {
        // docker run -it --rm -e POSTGRES_PASSWORD=postgres -p 5432:5432
        // postgres
        let client = Client::connect("postgresql://postgres:postgres@localhost", NoTls).unwrap();
        let client = Mutex::new(client);

        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);

            // By default generates 256 random values per bit size. Configurable
            // with the `PROPTEST_CASES` env variable.
            let mut config = ProptestConfig::default();
            // No point in running many values for small sizes
            if BITS < 4 { config.cases = 16; }

            proptest!(config, |(value: Uint<BITS, LIMBS>)| {

                // Test based on which types value will fit
                let bits = value.bit_len();
                if bits <= 1 {
                    test_to(&client, value, &Type::BOOL);
                }
                if bits <= 15 {
                    test_to(&client, value, &Type::INT2);
                }
                if bits <= 31 {
                    test_to(&client, value, &Type::INT4);
                }
                if bits <= 32 {
                    test_to(&client, value, &Type::OID);
                }
                if bits <= 50 {
                    test_to(&client, value, &Type::MONEY);
                }
                if bits <= 63 {
                    test_to(&client, value, &Type::INT8);
                }

                // Floating points always work, except when the exponent
                // overflows. We map that to +∞, mut SQL rejects it. This
                // is handled in the `test_to` function.
                test_to(&client, value, &Type::FLOAT4);
                test_to(&client, value, &Type::FLOAT8);

                // Types that work for any size
                for ty in &[Type::NUMERIC, Type::BIT, Type::VARBIT, Type::BYTEA, Type::CHAR, Type::TEXT, Type::VARCHAR, Type::JSON, Type::JSONB] {
                    test_to(&client, value, ty);
                }

            });
        });
    }

    fn from_sql_error(ty: Type, raw: &[u8]) -> FromSqlError {
        *Uint::<64, 1>::from_sql(&ty, raw)
            .unwrap_err()
            .downcast::<FromSqlError>()
            .unwrap()
    }

    #[test]
    fn test_from_sql_numeric_max_weight() {
        let raw = [0x00, 0x01, 0x7f, 0xff, 0, 0, 0, 0, 0x03, 0xe8];
        assert!(Uint::<256, 4>::from_sql(&Type::NUMERIC, &raw).is_err());
    }

    #[test]
    fn test_from_sql_bit_length_mismatch() {
        for ty in [Type::BIT, Type::VARBIT] {
            let parse_error = FromSqlError::ParseError(ty.clone());
            assert_eq!(
                Uint::<64, 1>::from_sql(&ty, &[0, 0, 0, 3, 0b1010_0000]).unwrap(),
                Uint::<64, 1>::from(5)
            );
            assert_eq!(from_sql_error(ty.clone(), &[0, 0, 0, 3]), parse_error);
            assert_eq!(
                from_sql_error(ty.clone(), &[0, 0, 0, 1, 0xff, 0xff]),
                parse_error
            );
            assert_eq!(from_sql_error(ty, &[0, 0, 0, 9, 0xff]), parse_error);
        }
    }

    #[test]
    fn test_from_sql_jsonb_empty() {
        assert_eq!(
            from_sql_error(Type::JSONB, &[]),
            FromSqlError::ParseError(Type::JSONB)
        );
    }

    #[test]
    fn test_from_sql_json_lone_quote() {
        assert!(Uint::<64, 1>::from_sql(&Type::JSON, b"\"").is_err());
        assert!(Uint::<64, 1>::from_sql(&Type::JSONB, &[1, b'"']).is_err());
        assert_eq!(
            Uint::<64, 1>::from_sql(&Type::JSON, b"\"12\"").unwrap(),
            Uint::from(12)
        );
    }

    fn int_to_sql<const BITS: usize, const LIMBS: usize>(
        value: Int<BITS, LIMBS>,
        ty: &Type,
    ) -> Result<Vec<u8>, BoxedError> {
        let mut out = BytesMut::new();
        value.to_sql(ty, &mut out)?;
        Ok(out.to_vec())
    }

    #[test]
    fn positive_i256_from_sql() {
        assert_eq!(
            I256::from_sql(&Type::NUMERIC, &[
                0x00, 0x01, // ndigits: 1
                0x00, 0x00, // weight: 0
                0x00, 0x00, // sign: 0x0000 (positive)
                0x00, 0x00, // scale: 0
                0x00, 0x01, // digit: 1
            ])
            .unwrap(),
            I256::ONE
        );
    }

    #[test]
    fn positive_i256_to_sql() {
        assert_eq!(int_to_sql(I256::ONE, &Type::NUMERIC).unwrap(), [
            0x00, 0x01, // ndigits: 1
            0x00, 0x00, // weight: 0
            0x00, 0x00, // sign: 0x0000 (positive)
            0x00, 0x00, // scale: 0
            0x00, 0x01, // digit: 1
        ],);
    }

    #[test]
    fn negative_i256_from_sql() {
        assert_eq!(
            I256::from_sql(&Type::NUMERIC, &[
                0x00, 0x01, // ndigits: 1
                0x00, 0x00, // weight: 0
                0x40, 0x00, // sign: 0x4000 (negative)
                0x00, 0x00, // scale: 0
                0x00, 0x01, // digit: 1
            ])
            .unwrap(),
            I256::MINUS_ONE
        );
    }

    #[test]
    fn negative_i256_to_sql() {
        assert_eq!(int_to_sql(I256::MINUS_ONE, &Type::NUMERIC).unwrap(), [
            0x00, 0x01, // ndigits: 1
            0x00, 0x00, // weight: 0
            0x40, 0x00, // sign: 0x4000 (negative)
            0x00, 0x00, // scale: 0
            0x00, 0x01, // digit: 1
        ],);
    }

    #[test]
    fn test_int_basic() {
        let min_hex = format!("-0x8{}", "0".repeat(63));
        let max_hex = format!("0x7{}", "f".repeat(63));
        for (value, text) in [
            (I256::MINUS_ONE, "-0x1"),
            (I256::ZERO, "0x0"),
            (I256::MIN, &min_hex),
            (I256::MAX, &max_hex),
        ] {
            for ty in [Type::CHAR, Type::TEXT, Type::VARCHAR] {
                assert_eq!(int_to_sql(value, &ty).unwrap(), text.as_bytes());
            }
            assert_eq!(
                int_to_sql(value, &Type::JSON).unwrap(),
                format!("\"{text}\"").as_bytes()
            );
            assert_eq!(
                int_to_sql(value, &Type::JSONB).unwrap(),
                [&[1], format!("\"{text}\"").as_bytes()].concat()
            );
        }
        assert_eq!(
            int_to_sql(I256::MINUS_ONE, &Type::BYTEA).unwrap(),
            [0xff; 32]
        );
        assert_eq!(
            int_to_sql(I256::MIN, &Type::BIT).unwrap(),
            hex!("000001008000000000000000000000000000000000000000000000000000000000000000")
        );
        let mut numeric = BytesMut::new();
        I256::MIN
            .unsigned_abs()
            .to_sql(&Type::NUMERIC, &mut numeric)
            .unwrap();
        numeric[4] = 0x40;
        assert_eq!(int_to_sql(I256::MIN, &Type::NUMERIC).unwrap(), numeric);
        assert_eq!(
            int_to_sql(I256::MIN, &Type::FLOAT8).unwrap(),
            (-(2.0_f64.powi(255))).to_be_bytes()
        );

        for (text, value) in [
            ("-1", I256::MINUS_ONE),
            ("-0x1", I256::MINUS_ONE),
            ("+0b11", I256::try_from(3).unwrap()),
            (
                "-57896044618658097711785492504343953926634992332820282019728792003956564819968",
                I256::MIN,
            ),
        ] {
            assert_eq!(I256::from_sql(&Type::TEXT, text.as_bytes()).unwrap(), value);
            assert_eq!(I256::from_sql(&Type::JSON, text.as_bytes()).unwrap(), value);
            let json = format!("\"{text}\"");
            assert_eq!(I256::from_sql(&Type::JSON, json.as_bytes()).unwrap(), value);
            assert_eq!(
                I256::from_sql(&Type::JSONB, &[&[1], json.as_bytes()].concat()).unwrap(),
                value
            );
        }
        assert!(I256::from_sql(&Type::TEXT, max_hex.replace("0x7", "0x8").as_bytes()).is_err());
        assert!(
            I256::from_sql(&Type::TEXT, format!("-0x8{}1", "0".repeat(62)).as_bytes()).is_err()
        );
        assert!(I256::from_sql(&Type::TEXT, b"\"1\"").is_err());
    }

    #[test]
    fn test_int_integer_types() {
        fn check<T: ToSql + for<'a> FromSql<'a> + Copy + Into<i128>>(ty: &Type, min: T, max: T) {
            let (min, max) = (min.into(), max.into());
            for value in [min, max, -1, 0, 1] {
                let int = I256::try_from(value).unwrap();
                let mut expected = BytesMut::new();
                T::from_sql(ty, &int_to_sql(int, ty).unwrap())
                    .unwrap()
                    .to_sql(ty, &mut expected)
                    .unwrap();
                assert_eq!(int_to_sql(int, ty).unwrap(), expected);
                assert_eq!(I256::from_sql(ty, &expected).unwrap(), int);
            }
            for value in [min - 1, max + 1] {
                assert!(int_to_sql(I256::try_from(value).unwrap(), ty).is_err());
            }
        }
        check(&Type::INT2, i16::MIN, i16::MAX);
        check(&Type::INT4, i32::MIN, i32::MAX);
        check(&Type::INT8, i64::MIN, i64::MAX);

        let minus_one = I256::MINUS_ONE;
        assert_eq!(int_to_sql(minus_one, &Type::INT2).unwrap(), [0xff; 2]);
        assert_eq!(int_to_sql(minus_one, &Type::INT8).unwrap(), [0xff; 8]);
        assert!(int_to_sql(minus_one, &Type::OID).is_err());
        let u32_max = I256::try_from(u32::MAX).unwrap();
        assert_eq!(int_to_sql(u32_max, &Type::OID).unwrap(), [0xff; 4]);
        assert_eq!(I256::from_sql(&Type::OID, &[0xff; 4]).unwrap(), u32_max);
        assert!(int_to_sql(u32_max + I256::ONE, &Type::OID).is_err());

        assert_eq!(int_to_sql(I256::ONE, &Type::BOOL).unwrap(), [1]);
        assert_eq!(int_to_sql(I256::ZERO, &Type::BOOL).unwrap(), [0]);
        assert!(int_to_sql(minus_one, &Type::BOOL).is_err());
        assert!(int_to_sql(I256::try_from(2).unwrap(), &Type::BOOL).is_err());

        let money = int_to_sql(I256::try_from(-5).unwrap(), &Type::MONEY).unwrap();
        assert_eq!(money, (-500_i64).to_be_bytes());
        assert_eq!(
            I256::from_sql(&Type::MONEY, &money).unwrap(),
            I256::try_from(-5).unwrap()
        );
        assert_eq!(
            I256::from_sql(&Type::MONEY, &(-599_i64).to_be_bytes()).unwrap(),
            I256::try_from(-5).unwrap()
        );
        assert!(int_to_sql(I256::try_from(i64::MIN / 100 - 1).unwrap(), &Type::MONEY).is_err());

        assert_eq!(
            I8::from_sql(&Type::INT2, &(-128_i16).to_be_bytes()).unwrap(),
            I8::MIN
        );
        assert!(I8::from_sql(&Type::INT2, &(-129_i16).to_be_bytes()).is_err());
        assert!(I8::from_sql(&Type::INT2, &128_i16.to_be_bytes()).is_err());
        assert!(I8::from_sql(&Type::FLOAT8, &(-128.6_f64).to_be_bytes()).is_err());
        assert!(I8::from_sql(&Type::FLOAT8, &f64::NAN.to_be_bytes()).is_err());
        assert_eq!(
            I8::from_sql(&Type::FLOAT4, &(-127.6_f32).to_be_bytes()).unwrap(),
            I8::MIN
        );
    }

    #[test]
    fn test_int_numeric_bounds() {
        fn numeric(sign: Sign, abs: Uint<256, 4>) -> Vec<u8> {
            let mut out = BytesMut::new();
            numeric_to_sql(sign, &abs, &mut out).unwrap();
            out.to_vec()
        }
        let half = Uint::<256, 4>::ONE << 255;
        assert_eq!(
            I256::from_sql(&Type::NUMERIC, &numeric(Sign::Negative, half)).unwrap(),
            I256::MIN
        );
        assert_eq!(
            I256::from_sql(&Type::NUMERIC, &numeric(Sign::Positive, half - Uint::ONE)).unwrap(),
            I256::MAX
        );
        for raw in [
            numeric(Sign::Positive, half),
            numeric(Sign::Negative, half + Uint::ONE),
        ] {
            let error = I256::from_sql(&Type::NUMERIC, &raw).unwrap_err();
            assert_eq!(
                *error.downcast::<FromSqlError>().unwrap(),
                FromSqlError::Overflow
            );
        }
        assert_eq!(
            I256::from_sql(&Type::NUMERIC, &numeric(Sign::Negative, Uint::ZERO)).unwrap(),
            I256::ZERO
        );
        // NaN, +Infinity and -Infinity.
        for sign in [0xc000_u16, 0xd000, 0xf000] {
            let mut raw = numeric(Sign::Positive, Uint::ZERO);
            raw[4..6].copy_from_slice(&sign.to_be_bytes());
            let error = I256::from_sql(&Type::NUMERIC, &raw).unwrap_err();
            assert_eq!(
                *error.downcast::<FromSqlError>().unwrap(),
                FromSqlError::ParseError(Type::NUMERIC)
            );
        }
        // `Uint` rejects negative values.
        assert_eq!(
            from_sql_error(Type::NUMERIC, &numeric(Sign::Negative, Uint::ONE)),
            FromSqlError::ParseError(Type::NUMERIC)
        );
    }

    #[test]
    fn test_int_roundtrip() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(value: I)| {
                for value in [value, I::MIN, I::MAX, I::MINUS_ONE, I::wrapping_from(value.wrapping_to::<i16>())] {
                    for ty in &[Type::BOOL, Type::INT2, Type::INT4, Type::INT8, Type::OID, Type::MONEY, Type::NUMERIC, Type::BYTEA, Type::BIT, Type::VARBIT, Type::CHAR, Type::TEXT, Type::VARCHAR, Type::JSON, Type::JSONB] {
                        if let Ok(serialized) = int_to_sql(value, ty) {
                            assert_eq!(I::from_sql(ty, &serialized).unwrap(), value, "{ty}");
                        }
                    }
                    if f64::from(value).is_finite() {
                        let deserialized = I::from_sql(&Type::FLOAT8, &int_to_sql(value, &Type::FLOAT8).unwrap());
                        if let Ok(deserialized) = deserialized {
                            assert_ulps_eq!(f64::from(value), f64::from(deserialized), max_ulps = 4);
                        }
                    }
                }
            });
        });
    }

    fn test_to_int<const BITS: usize, const LIMBS: usize>(
        client: &Mutex<Client>,
        value: Int<BITS, LIMBS>,
        ty: &Type,
    ) {
        println!("testing {value:?} {ty}");

        // Skip values that are out of range for the type
        let Ok(serialized) = int_to_sql(value, ty) else {
            return;
        };
        // Skip floating point infinities
        if (ty == &Type::FLOAT4 && f32::from(value).is_infinite())
            || (ty == &Type::FLOAT8 && f64::from(value).is_infinite())
        {
            return;
        }

        // Fetch ground truth value from Postgres
        let raw = value.cast_unsigned();
        let text = format!(
            "{}{:#x}",
            if value.is_negative() { "-" } else { "" },
            value.unsigned_abs()
        );
        let expr = match *ty {
            Type::BIT | Type::VARBIT if BITS == 0 => "B''::varbit".to_string(),
            Type::BIT => format!("B'{raw:0BITS$b}'::bit({BITS})"),
            Type::VARBIT => format!("B'{raw:0BITS$b}'::varbit"),
            Type::BYTEA if BITS == 0 => "'\\x'::bytea".to_string(),
            Type::BYTEA => format!("'\\x{raw:0w$x}'::bytea", w = 2 * nbytes(BITS)),
            Type::CHAR => format!("'{text}'::char({})", text.len()),
            Type::TEXT | Type::VARCHAR => format!("'{text}'::{}", ty.name()),
            Type::JSON | Type::JSONB => format!("'\"{text}\"'::{}", ty.name()),
            _ => format!("'{value}'::{}", ty.name()),
        };
        let ground_truth = {
            let mut client = client.lock().unwrap();
            get_binary(&mut client, &expr)
        };

        // Compare with ground truth, for float we allow tiny rounding error
        if ty == &Type::FLOAT4 {
            let serialized = f32::from_be_bytes(serialized.try_into().unwrap());
            let ground_truth = f32::from_be_bytes(ground_truth.try_into().unwrap());
            assert_ulps_eq!(serialized, ground_truth, max_ulps = 4);
        } else if ty == &Type::FLOAT8 {
            let serialized = f64::from_be_bytes(serialized.try_into().unwrap());
            let ground_truth = f64::from_be_bytes(ground_truth.try_into().unwrap());
            assert_ulps_eq!(serialized, ground_truth, max_ulps = 4);
        } else {
            // Check that the value is exactly the same as the ground truth
            assert_eq!(serialized, ground_truth);
            assert_eq!(Int::from_sql(ty, &ground_truth).unwrap(), value);
        }
    }

    // This test requires a live postgresql server, see `test_postgres`.
    #[test]
    #[ignore = "requires a live postgresql server"]
    fn test_postgres_int() {
        let client = Client::connect("postgresql://postgres:postgres@localhost", NoTls).unwrap();
        let client = Mutex::new(client);

        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;

            let mut config = ProptestConfig::default();
            if BITS < 4 { config.cases = 16; }

            proptest!(config, |(value: I)| {
                let small = [i8::MIN.into(), i16::MIN.into(), i32::MIN.into(), i64::MIN]
                    .map(|min| I::wrapping_from(value.wrapping_to::<i64>() % min));
                for value in [value, I::MIN, I::MAX, I::MINUS_ONE, I::ZERO, I::ONE].into_iter().chain(small) {
                    for ty in &[Type::BOOL, Type::INT2, Type::INT4, Type::OID, Type::MONEY, Type::INT8, Type::FLOAT4, Type::FLOAT8, Type::NUMERIC, Type::BIT, Type::VARBIT, Type::BYTEA, Type::CHAR, Type::TEXT, Type::VARCHAR, Type::JSON, Type::JSONB] {
                        test_to_int(&client, value, ty);
                    }
                }
            });
        });
    }
}
