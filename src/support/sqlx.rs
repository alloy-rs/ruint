//! Support for the [`sqlx`](https://crates.io/crates/sqlx) crate.
//!
//! Currently only encodes to/from a big-endian byte array. [`Int`] uses its
//! two's complement representation. With the `sqlx-postgres` feature,
//! `Vec<Uint>` and `Vec<Int>` map to `BYTEA[]`.

#![cfg(feature = "sqlx")]
#![cfg_attr(docsrs, doc(cfg(feature = "sqlx")))]

use sqlx_core::{
    database::Database,
    decode::Decode,
    encode::{Encode, IsNull},
    error::BoxDynError,
    types::Type,
};
use thiserror::Error;

#[cfg(feature = "sqlx-postgres")]
use sqlx_postgres::{PgHasArrayType, PgTypeInfo};

use crate::{Int, Uint};

#[derive(Error, Debug)]
pub enum DecodeError {
    #[error("Value too large for target type")]
    Overflow,
}

impl<const BITS: usize, const LIMBS: usize, DB: Database> Type<DB> for Uint<BITS, LIMBS>
where
    Vec<u8>: Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <Vec<u8> as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <Vec<u8> as Type<DB>>::compatible(ty)
    }
}

impl<'a, const BITS: usize, const LIMBS: usize, DB: Database> Encode<'a, DB> for Uint<BITS, LIMBS>
where
    Vec<u8>: Encode<'a, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as Database>::ArgumentBuffer<'a>,
    ) -> Result<IsNull, BoxDynError> {
        self.to_be_bytes_vec().encode_by_ref(buf)
    }
}

impl<'a, const BITS: usize, const LIMBS: usize, DB: Database> Decode<'a, DB> for Uint<BITS, LIMBS>
where
    Vec<u8>: Decode<'a, DB>,
{
    fn decode(value: <DB as Database>::ValueRef<'a>) -> Result<Self, BoxDynError> {
        let bytes = Vec::<u8>::decode(value)?;
        Self::try_from_be_slice(bytes.as_slice()).ok_or_else(|| DecodeError::Overflow.into())
    }
}

impl<const BITS: usize, const LIMBS: usize, DB: Database> Type<DB> for Int<BITS, LIMBS>
where
    Vec<u8>: Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <Vec<u8> as Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <Vec<u8> as Type<DB>>::compatible(ty)
    }
}

impl<'a, const BITS: usize, const LIMBS: usize, DB: Database> Encode<'a, DB> for Int<BITS, LIMBS>
where
    Vec<u8>: Encode<'a, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as Database>::ArgumentBuffer<'a>,
    ) -> Result<IsNull, BoxDynError> {
        self.cast_unsigned().encode_by_ref(buf)
    }
}

impl<'a, const BITS: usize, const LIMBS: usize, DB: Database> Decode<'a, DB> for Int<BITS, LIMBS>
where
    Vec<u8>: Decode<'a, DB>,
{
    fn decode(value: <DB as Database>::ValueRef<'a>) -> Result<Self, BoxDynError> {
        Uint::decode(value).map(Uint::cast_signed)
    }
}

#[cfg(feature = "sqlx-postgres")]
impl<const BITS: usize, const LIMBS: usize> PgHasArrayType for Uint<BITS, LIMBS> {
    fn array_type_info() -> PgTypeInfo {
        <Vec<u8> as PgHasArrayType>::array_type_info()
    }
}

#[cfg(feature = "sqlx-postgres")]
impl<const BITS: usize, const LIMBS: usize> PgHasArrayType for Int<BITS, LIMBS> {
    fn array_type_info() -> PgTypeInfo {
        <Vec<u8> as PgHasArrayType>::array_type_info()
    }
}

#[cfg(all(test, feature = "sqlx-postgres"))]
mod tests {
    use super::*;
    use crate::aliases::{I8, I256, U256};
    use sqlx_postgres::{PgArgumentBuffer, Postgres};

    fn encode_pg<T: for<'q> Encode<'q, Postgres>>(value: &T) -> Vec<u8> {
        let mut buf = PgArgumentBuffer::default();
        let _ = value.encode_by_ref(&mut buf).unwrap();
        buf.to_vec()
    }

    #[test]
    fn test_int_pg_type() {
        let expected = <Vec<u8> as Type<Postgres>>::type_info();
        assert_eq!(format!("{:?}", I256::type_info()), format!("{expected:?}"));

        let expected = <Vec<Vec<u8>> as Type<Postgres>>::type_info();
        assert_eq!(
            format!("{:?}", <Vec<U256> as Type<Postgres>>::type_info()),
            format!("{expected:?}")
        );
        let actual = <Vec<I256> as Type<Postgres>>::type_info();
        assert_eq!(actual, expected);
        // `PartialEq` treats `array_of("bytea")` as compatible; Debug pins the
        // built-in OID.
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    }

    #[test]
    fn test_int_pg_encode() {
        for (value, bytes) in [
            (I256::ZERO, [0x00; 32]),
            (I256::ONE, I256::ONE.to_be_bytes()),
            (I256::MINUS_ONE, [0xff; 32]),
            (I256::MIN, I256::MIN.cast_unsigned().to_be_bytes()),
            (I256::MAX, I256::MAX.cast_unsigned().to_be_bytes()),
        ] {
            assert_eq!(encode_pg(&value), encode_pg(&bytes.to_vec()));
        }
        assert_eq!(encode_pg(&I8::MIN), encode_pg(&vec![0x80_u8]));

        let signed = vec![I256::ONE, I256::MINUS_ONE, I256::ZERO, I256::MIN, I256::MAX];
        assert_eq!(
            encode_pg(&signed),
            encode_pg(
                &signed
                    .iter()
                    .map(|s| s.to_be_bytes::<32>().to_vec())
                    .collect::<Vec<_>>()
            )
        );
    }
}
