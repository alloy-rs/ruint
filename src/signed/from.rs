use crate::{FromUintError, Int, Sign, ToUintError, Uint, UintTryFrom, UintTryTo};
use core::fmt::Debug;

impl<const BITS: usize, const LIMBS: usize> Int<BITS, LIMBS> {
    /// Construct a new [`Int`] from the value.
    ///
    /// # Panics
    ///
    /// Panics if the conversion fails, for example if the value does not fit
    /// in the [`Int`]. The panic will be attributed to the call site.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::{Int, uint, aliases::*};
    /// # uint!{
    /// assert_eq!(I8::from(-128_i16), -128_I8);
    /// assert_eq!(I64::from(-0x7014b4c2d1f2_I256), -0x7014b4c2d1f2_I64);
    /// assert_eq!(I64::from(0x7014b4c2d1f2_U256), 0x7014b4c2d1f2_I64);
    /// assert_eq!(I64::from(-3.9), -4_I64);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn from<T>(value: T) -> Self
    where
        Self: UintTryFrom<T>,
    {
        match Self::uint_try_from(value) {
            Ok(n) => n,
            Err(e) => panic!("Int conversion error: {e}"),
        }
    }

    /// Construct a new [`Int`] from the value, saturating at the numeric
    /// bounds of the [`Int`].
    ///
    /// If the value is not a number (like `f64::NAN`), then the result is
    /// zero.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::{Int, uint, aliases::*};
    /// # uint!{
    /// assert_eq!(I8::saturating_from(300_u16), I8::MAX);
    /// assert_eq!(I8::saturating_from(-300_i16), I8::MIN);
    /// assert_eq!(I32::saturating_from(-0x7014b4c2d1f2_I256), I32::MIN);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn saturating_from<T>(value: T) -> Self
    where
        Self: UintTryFrom<T>,
    {
        match Self::uint_try_from(value) {
            Ok(n) => n,
            Err(ToUintError::ValueTooLarge(..)) => Self::MAX,
            Err(ToUintError::ValueNegative(..)) => Self::MIN,
            Err(ToUintError::NotANumber(_)) => Self::ZERO,
        }
    }

    /// Construct a new [`Int`] from the value, wrapping it modulo
    /// $2^{\mathtt{BITS}}$.
    ///
    /// If the value is not a number (like `f64::NAN`), then the result is
    /// zero.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::{Int, uint, aliases::*};
    /// # uint!{
    /// assert_eq!(I8::wrapping_from(300_u16), 44_I8);
    /// assert_eq!(I8::wrapping_from(-300_i16), -44_I8);
    /// assert_eq!(I32::wrapping_from(0x7014b4c2d1f2_U256), -0x4b3d2e0e_I32);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn wrapping_from<T>(value: T) -> Self
    where
        Self: UintTryFrom<T>,
    {
        match Self::uint_try_from(value) {
            Ok(n) | Err(ToUintError::ValueTooLarge(_, n) | ToUintError::ValueNegative(_, n)) => n,
            Err(ToUintError::NotANumber(_)) => Self::ZERO,
        }
    }

    /// Converts `self` to the target type.
    ///
    /// # Panics
    ///
    /// Panics if the conversion fails, for example if the value does not fit
    /// in the target type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::{Int, uint, aliases::*};
    /// # uint!{
    /// assert_eq!((-300_I12).to::<i16>(), -300_i16);
    /// assert_eq!((-300_I12).to::<I256>(), -300_I256);
    /// assert_eq!(300_I12.to::<U256>(), 300_U256);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[track_caller]
    pub fn to<T>(&self) -> T
    where
        Self: UintTryTo<T>,
        T: Debug,
    {
        self.uint_try_to().expect("Int conversion error")
    }

    /// Converts `self` to the target type, wrapping it modulo the size of the
    /// target type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::{Int, uint, aliases::*};
    /// # uint!{
    /// assert_eq!((-300_I12).wrapping_to::<i8>(), -44_i8);
    /// assert_eq!((-1_I32).wrapping_to::<u8>(), 255_u8);
    /// assert_eq!((-1_I32).wrapping_to::<U64>(), U64::MAX);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn wrapping_to<T>(&self) -> T
    where
        Self: UintTryTo<T>,
    {
        match self.uint_try_to() {
            Ok(n) | Err(FromUintError::Overflow(_, n, _)) => n,
        }
    }

    /// Converts `self` to the target type, saturating at the numeric bounds of
    /// the target type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ruint::{Int, uint, aliases::*};
    /// # uint!{
    /// assert_eq!((-300_I12).saturating_to::<i8>(), i8::MIN);
    /// assert_eq!((-1_I32).saturating_to::<u8>(), 0);
    /// assert_eq!(I256::MAX.saturating_to::<I64>(), I64::MAX);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn saturating_to<T>(&self) -> T
    where
        Self: UintTryTo<T>,
    {
        match self.uint_try_to() {
            Ok(n) | Err(FromUintError::Overflow(_, _, n)) => n,
        }
    }

    /// Converts a sign and a possibly truncated absolute value.
    ///
    /// Values below [`Int::MIN`] are reported as
    /// [`ToUintError::ValueNegative`].
    #[inline]
    fn try_from_sign_and_abs(
        sign: Sign,
        abs: Result<Uint<BITS, LIMBS>, ToUintError<Uint<BITS, LIMBS>>>,
    ) -> Result<Self, ToUintError<Self>> {
        let (abs, truncated) = match abs {
            Ok(abs) => (abs, false),
            Err(ToUintError::ValueTooLarge(_, abs) | ToUintError::ValueNegative(_, abs)) => {
                (abs, true)
            }
            Err(ToUintError::NotANumber(bits)) => return Err(ToUintError::NotANumber(bits)),
        };
        // Negating the truncated absolute value gives the truncated value.
        match Self::overflowing_from_sign_and_abs(sign, abs) {
            (value, false) if !truncated => Ok(value),
            (value, _) => Err(match sign {
                Sign::Positive => ToUintError::ValueTooLarge(BITS, value),
                Sign::Negative => ToUintError::ValueNegative(BITS, value),
            }),
        }
    }
}

/// Blanket implementation for any type that implements [`TryFrom<Int>`].
impl<const BITS: usize, const LIMBS: usize, T> UintTryFrom<T> for Int<BITS, LIMBS>
where
    Self: TryFrom<T, Error = ToUintError<Self>>,
{
    #[inline]
    fn uint_try_from(value: T) -> Result<Self, ToUintError<Self>> {
        Self::try_from(value)
    }
}

impl<const BITS: usize, const LIMBS: usize, const BITS_SRC: usize, const LIMBS_SRC: usize>
    UintTryFrom<Int<BITS_SRC, LIMBS_SRC>> for Int<BITS, LIMBS>
{
    #[inline]
    fn uint_try_from(value: Int<BITS_SRC, LIMBS_SRC>) -> Result<Self, ToUintError<Self>> {
        let (sign, abs) = value.into_sign_and_abs();
        Self::try_from_sign_and_abs(sign, Uint::uint_try_from(abs))
    }
}

impl<const BITS: usize, const LIMBS: usize, const BITS_SRC: usize, const LIMBS_SRC: usize>
    UintTryFrom<Uint<BITS_SRC, LIMBS_SRC>> for Int<BITS, LIMBS>
{
    #[inline]
    fn uint_try_from(value: Uint<BITS_SRC, LIMBS_SRC>) -> Result<Self, ToUintError<Self>> {
        Self::try_from_sign_and_abs(Sign::Positive, Uint::uint_try_from(value))
    }
}

impl<const BITS: usize, const LIMBS: usize, const BITS_SRC: usize, const LIMBS_SRC: usize>
    UintTryFrom<Int<BITS_SRC, LIMBS_SRC>> for Uint<BITS, LIMBS>
{
    #[inline]
    fn uint_try_from(value: Int<BITS_SRC, LIMBS_SRC>) -> Result<Self, ToUintError<Self>> {
        if value.is_negative() {
            let wrapped = Self::wrapping_from(value.unsigned_abs()).wrapping_neg();
            Err(ToUintError::ValueNegative(BITS, wrapped))
        } else {
            Self::uint_try_from(value.0)
        }
    }
}

/// Blanket implementation for any type that implements [`TryFrom<&Int>`].
impl<const BITS: usize, const LIMBS: usize, T> UintTryTo<T> for Int<BITS, LIMBS>
where
    T: for<'a> TryFrom<&'a Self, Error = FromUintError<T>>,
{
    #[inline]
    fn uint_try_to(&self) -> Result<T, FromUintError<T>> {
        T::try_from(self)
    }
}

impl<const BITS: usize, const LIMBS: usize, const BITS_DST: usize, const LIMBS_DST: usize>
    UintTryTo<Int<BITS_DST, LIMBS_DST>> for Int<BITS, LIMBS>
{
    #[inline]
    fn uint_try_to(
        &self,
    ) -> Result<Int<BITS_DST, LIMBS_DST>, FromUintError<Int<BITS_DST, LIMBS_DST>>> {
        Int::uint_try_from(*self).map_err(|e| to_from_error(BITS, e))
    }
}

impl<const BITS: usize, const LIMBS: usize, const BITS_DST: usize, const LIMBS_DST: usize>
    UintTryTo<Uint<BITS_DST, LIMBS_DST>> for Int<BITS, LIMBS>
{
    #[inline]
    fn uint_try_to(
        &self,
    ) -> Result<Uint<BITS_DST, LIMBS_DST>, FromUintError<Uint<BITS_DST, LIMBS_DST>>> {
        Uint::uint_try_from(*self).map_err(|e| match e {
            ToUintError::ValueTooLarge(_, n) => FromUintError::Overflow(BITS, n, Uint::MAX),
            ToUintError::ValueNegative(_, n) => FromUintError::Overflow(BITS, n, Uint::ZERO),
            ToUintError::NotANumber(_) => unreachable!(),
        })
    }
}

impl<const BITS: usize, const LIMBS: usize, const BITS_DST: usize, const LIMBS_DST: usize>
    UintTryTo<Int<BITS_DST, LIMBS_DST>> for Uint<BITS, LIMBS>
{
    #[inline]
    fn uint_try_to(
        &self,
    ) -> Result<Int<BITS_DST, LIMBS_DST>, FromUintError<Int<BITS_DST, LIMBS_DST>>> {
        Int::uint_try_from(*self).map_err(|e| to_from_error(BITS, e))
    }
}

/// Converts a [`ToUintError`] of an [`Int`] conversion to a [`FromUintError`]
/// with the saturated value.
#[inline]
fn to_from_error<const BITS: usize, const LIMBS: usize>(
    bits: usize,
    error: ToUintError<Int<BITS, LIMBS>>,
) -> FromUintError<Int<BITS, LIMBS>> {
    match error {
        ToUintError::ValueTooLarge(_, n) => FromUintError::Overflow(bits, n, Int::MAX),
        ToUintError::ValueNegative(_, n) => FromUintError::Overflow(bits, n, Int::MIN),
        ToUintError::NotANumber(_) => unreachable!(),
    }
}

macro_rules! impl_from_unsigned_int {
    ($($uint:ty)*) => {$(
        impl<const BITS: usize, const LIMBS: usize> TryFrom<$uint> for Int<BITS, LIMBS> {
            type Error = ToUintError<Self>;

            #[inline]
            fn try_from(value: $uint) -> Result<Self, Self::Error> {
                Self::try_from_sign_and_abs(Sign::Positive, Uint::try_from(value))
            }
        }
    )*};
}

impl_from_unsigned_int!(bool u8 u16 u32 u64 u128 usize);

macro_rules! impl_from_signed_int {
    ($($int:ty)*) => {$(
        impl<const BITS: usize, const LIMBS: usize> TryFrom<$int> for Int<BITS, LIMBS> {
            type Error = ToUintError<Self>;

            #[inline]
            fn try_from(value: $int) -> Result<Self, Self::Error> {
                let sign = if value < 0 { Sign::Negative } else { Sign::Positive };
                Self::try_from_sign_and_abs(sign, Uint::try_from(value.unsigned_abs()))
            }
        }
    )*};
}

impl_from_signed_int!(i8 i16 i32 i64 i128 isize);

impl<const BITS: usize, const LIMBS: usize> TryFrom<f64> for Int<BITS, LIMBS> {
    type Error = ToUintError<Self>;

    /// Converts `value` rounding to the nearest integer, ties to even.
    #[inline]
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let sign = if value.is_sign_negative() {
            Sign::Negative
        } else {
            Sign::Positive
        };
        Self::try_from_sign_and_abs(sign, Uint::try_from(value.abs()))
    }
}

impl<const BITS: usize, const LIMBS: usize> TryFrom<f32> for Int<BITS, LIMBS> {
    type Error = ToUintError<Self>;

    /// Converts `value` rounding to the nearest integer, ties to even.
    #[inline]
    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::try_from(f64::from(value))
    }
}

macro_rules! to_int {
    ($($int:ty)*) => {$(
        impl<const BITS: usize, const LIMBS: usize> TryFrom<Int<BITS, LIMBS>> for $int {
            type Error = FromUintError<Self>;

            #[inline]
            fn try_from(value: Int<BITS, LIMBS>) -> Result<Self, Self::Error> {
                Self::try_from(&value)
            }
        }

        impl<const BITS: usize, const LIMBS: usize> TryFrom<&Int<BITS, LIMBS>> for $int {
            type Error = FromUintError<Self>;

            #[inline]
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            fn try_from(value: &Int<BITS, LIMBS>) -> Result<Self, Self::Error> {
                let (sign, abs) = value.into_sign_and_abs();
                let (abs, truncated) = match u128::try_from(&abs) {
                    Ok(abs) => (abs, false),
                    Err(FromUintError::Overflow(_, abs, _)) => (abs, true),
                };
                // Negating the truncated absolute value gives the truncated
                // value.
                let (wrapped, fits, saturated) = match sign {
                    Sign::Positive => (abs as Self, abs <= Self::MAX as u128, Self::MAX),
                    Sign::Negative => (
                        (abs as Self).wrapping_neg(),
                        abs <= (Self::MIN as i128).unsigned_abs(),
                        Self::MIN,
                    ),
                };
                if fits && !truncated {
                    Ok(wrapped)
                } else {
                    Err(FromUintError::Overflow(BITS, wrapped, saturated))
                }
            }
        }
    )*};
}

to_int!(i8 u8 i16 u16 i32 u32 i64 u64 i128 u128 isize usize);

impl<const BITS: usize, const LIMBS: usize> From<Int<BITS, LIMBS>> for f32 {
    #[inline]
    fn from(value: Int<BITS, LIMBS>) -> Self {
        Self::from(&value)
    }
}

impl<const BITS: usize, const LIMBS: usize> From<&Int<BITS, LIMBS>> for f32 {
    /// Approximate single precision float.
    ///
    /// Returns `f32::INFINITY` or `f32::NEG_INFINITY` if the value is too
    /// large to represent.
    #[inline]
    fn from(value: &Int<BITS, LIMBS>) -> Self {
        // Rounding is symmetric around zero, so negating the rounded absolute
        // value gives the rounded value.
        let abs = Self::from(value.unsigned_abs());
        if value.is_negative() { -abs } else { abs }
    }
}

impl<const BITS: usize, const LIMBS: usize> From<Int<BITS, LIMBS>> for f64 {
    #[inline]
    fn from(value: Int<BITS, LIMBS>) -> Self {
        Self::from(&value)
    }
}

impl<const BITS: usize, const LIMBS: usize> From<&Int<BITS, LIMBS>> for f64 {
    /// Approximate double precision float.
    ///
    /// Returns `f64::INFINITY` or `f64::NEG_INFINITY` if the value is too
    /// large to represent.
    #[inline]
    fn from(value: &Int<BITS, LIMBS>) -> Self {
        // Rounding is symmetric around zero, so negating the rounded absolute
        // value gives the rounded value.
        let abs = Self::from(value.unsigned_abs());
        if value.is_negative() { -abs } else { abs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aliases::{I0, I1, I8, I64, I128, I256, U8, U64, U256},
        const_for, nlimbs,
    };
    use proptest::proptest;

    #[test]
    fn test_small_exhaustive() {
        for a in i16::MIN..=i16::MAX {
            let expected = i8::try_from(a).ok();
            let x = I8::try_from(a);
            assert_eq!(x.ok().map(|x| i8::try_from(x).unwrap()), expected);
            assert_eq!(i8::try_from(I64::try_from(a).unwrap()).ok(), expected);
            assert_eq!(
                u8::try_from(I64::try_from(a).unwrap()).ok(),
                u8::try_from(a).ok()
            );
            assert_eq!(I8::wrapping_from(a), I8::try_from(a as i8).unwrap());
            assert_eq!(
                I8::saturating_from(a),
                I8::try_from(a.clamp(-128, 127) as i8).unwrap()
            );
            assert_eq!(I64::from(a).wrapping_to::<i8>(), a as i8);
            assert_eq!(I64::from(a).wrapping_to::<u8>(), a as u8);
            assert_eq!(I64::from(a).saturating_to::<i8>(), a.clamp(-128, 127) as i8);
            assert_eq!(I64::from(a).saturating_to::<u8>(), a.clamp(0, 255) as u8);
            assert_eq!(I64::from(a).wrapping_to::<I8>(), I8::from(a as i8));
            assert_eq!(
                I64::from(a).saturating_to::<I8>(),
                I8::from(a.clamp(-128, 127) as i8)
            );
            assert_eq!(I64::from(a).wrapping_to::<U8>(), U8::from(a as u8));
            assert_eq!(
                I64::from(a).saturating_to::<U8>(),
                U8::from(a.clamp(0, 255) as u8)
            );
            assert_eq!(
                U8::uint_try_from(I64::from(a)).ok(),
                u8::try_from(a).ok().map(U8::from)
            );
            assert_eq!(f64::from(I64::from(a)), f64::from(a));
            let f = f64::from(a) + if a < 0 { -0.25 } else { 0.25 };
            assert_eq!(I8::try_from(f).ok(), expected.map(I8::from));
        }
    }

    #[test]
    fn test_i128_reference() {
        proptest!(|(a: i128)| {
            let x = I128::try_from(a).unwrap();
            assert_eq!(i128::try_from(x), Ok(a));
            assert_eq!(x.wrapping_to::<i64>(), a as i64);
            assert_eq!(x.wrapping_to::<u64>(), a as u64);
            assert_eq!(x.wrapping_to::<u128>(), a as u128);
            assert_eq!(x.saturating_to::<i64>(), a.clamp(i64::MIN.into(), i64::MAX.into()) as i64);
            assert_eq!(I64::wrapping_from(x), I64::from(a as i64));
            assert_eq!(I256::from(x).to::<i128>(), a);
            assert_eq!(I256::from(a).wrapping_to::<U256>(), U256::from(a as u128) | if a < 0 { U256::MAX << 128 } else { U256::ZERO });
            assert_eq!(f64::from(x), a as f64);
            assert_eq!(f32::from(x), a as f32);
            let f = a as f64;
            assert_eq!(I128::try_from(f).ok().map(|x| x.to::<i128>()), (f < 2f64.powi(127)).then_some(f as i128));
        });
    }

    #[test]
    #[allow(clippy::absurd_extreme_comparisons, clippy::unnecessary_min_or_max)] // From macro.
    fn test_resize() {
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            type I = Int<BITS, LIMBS>;
            proptest!(|(value: I)| {
                let wide = I256::wrapping_from(value);
                if BITS <= 256 {
                    assert_eq!(wide.to::<I>(), value);
                    assert_eq!(I::uint_try_from(wide), Ok(value));
                }
                for i in 0..256 {
                    assert_eq!(wide.bit(i), BITS > 0 && value.bit(i.min(BITS - 1)));
                }
                assert_eq!(Int::<4096, 64>::from(value).to::<I>(), value);
                assert_eq!(value.wrapping_to::<Uint<BITS, LIMBS>>(), value.cast_unsigned());
                assert_eq!(value.cast_unsigned().wrapping_to::<I>(), value);
            });
        });
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(I0::try_from(0), Ok(I0::ZERO));
        assert_eq!(
            I0::try_from(1),
            Err(ToUintError::ValueTooLarge(0, I0::ZERO))
        );
        assert_eq!(
            I0::try_from(-1),
            Err(ToUintError::ValueNegative(0, I0::ZERO))
        );
        assert_eq!(I1::try_from(-1), Ok(I1::MINUS_ONE));
        assert_eq!(
            I1::try_from(1),
            Err(ToUintError::ValueTooLarge(1, I1::MINUS_ONE))
        );
        assert_eq!(
            I1::try_from(true),
            Err(ToUintError::ValueTooLarge(1, I1::MINUS_ONE))
        );
        assert_eq!(I8::try_from(true), Ok(I8::ONE));
        assert_eq!(
            I8::try_from(-129),
            Err(ToUintError::ValueNegative(8, I8::MAX))
        );
        assert_eq!(I8::try_from(f64::NAN), Err(ToUintError::NotANumber(8)));
        assert_eq!(I8::try_from(-0.0), Ok(I8::ZERO));
        assert_eq!(I8::try_from(-2.5), Ok(I8::from(-2)));
        assert_eq!(I8::try_from(-3.5), Ok(I8::from(-4)));
        assert_eq!(I8::try_from(-128.4), Ok(I8::MIN));
        assert_eq!(
            I8::try_from(-128.6),
            Err(ToUintError::ValueNegative(8, I8::MAX))
        );
        assert_eq!(I8::saturating_from(f64::NEG_INFINITY), I8::MIN);
        assert_eq!(I8::saturating_from(f64::INFINITY), I8::MAX);
        assert_eq!(I256::try_from(i128::MIN).unwrap().to::<i128>(), i128::MIN);
        assert_eq!(
            I128::try_from(u128::MAX),
            Err(ToUintError::ValueTooLarge(128, I128::MINUS_ONE))
        );
        assert_eq!(
            u64::try_from(I256::MINUS_ONE),
            Err(FromUintError::Overflow(256, u64::MAX, 0))
        );
        assert_eq!(
            i64::try_from(I256::MIN),
            Err(FromUintError::Overflow(256, 0, i64::MIN))
        );
        assert_eq!(
            I256::MIN.uint_try_to(),
            Err(FromUintError::Overflow(256, U64::ZERO, U64::ZERO))
        );
        assert_eq!(
            U256::MAX.uint_try_to(),
            Err(FromUintError::Overflow(256, I256::MINUS_ONE, I256::MAX))
        );
        assert_eq!(f64::from(I256::MIN), -(2f64.powi(255)));
        assert_eq!(f32::from(Int::<4096, 64>::MIN), f32::NEG_INFINITY);
    }
}
