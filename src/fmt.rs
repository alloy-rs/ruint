#![allow(clippy::missing_inline_in_public_items)] // allow format functions

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

use crate::Uint;
use core::{fmt, mem::MaybeUninit};

mod base {
    pub(super) trait Base {
        /// The base.
        const BASE: u64;
        /// The prefix for the base.
        const PREFIX: &'static str;
        /// Number of bits per digit. Only meaningful for power-of-2 bases.
        const BITS_PER_DIGIT: usize = 0;

        /// Highest power of the base that fits in a `u64`.
        const MAX: u64 = crate::utils::max_pow_u64(Self::BASE);
        /// Number of characters written using `MAX` as the base in
        /// `to_base_be`.
        const WIDTH: usize = Self::MAX.ilog(Self::BASE) as _;
    }

    pub(super) struct Binary;
    impl Base for Binary {
        const BASE: u64 = 2;
        const PREFIX: &'static str = "0b";
        const BITS_PER_DIGIT: usize = 1;
    }

    pub(super) struct Octal;
    impl Base for Octal {
        const BASE: u64 = 8;
        const PREFIX: &'static str = "0o";
        const BITS_PER_DIGIT: usize = 3;
    }

    pub(super) struct Decimal;
    impl Base for Decimal {
        const BASE: u64 = 10;
        const PREFIX: &'static str = "";
    }

    pub(super) struct Hexadecimal;
    impl Base for Hexadecimal {
        const BASE: u64 = 16;
        const PREFIX: &'static str = "0x";
        const BITS_PER_DIGIT: usize = 4;
    }
}
use base::Base;

#[cfg(feature = "alloc")]
#[allow(clippy::cast_possible_truncation)] // The result is at most `bits`.
const fn decimal_capacity(bits: usize) -> usize {
    // ceil(log10(2) * 2^32).
    const LOG10_2_Q32: u128 = 1_292_913_987;
    const SCALE: u128 = 1 << 32;

    let digits = (bits as u128 * LOG10_2_Q32 + SCALE - 1) >> 32;
    if digits == 0 { 1 } else { digits as usize }
}

macro_rules! impl_fmt_pow2 {
    ($tr:path; $base:ty, $upper:literal) => {
        impl<const BITS: usize, const LIMBS: usize> $tr for Uint<BITS, LIMBS> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if let Ok(small) = u64::try_from(self) {
                    return <u64 as $tr>::fmt(&small, f);
                }
                if let Ok(small) = u128::try_from(self) {
                    return <u128 as $tr>::fmt(&small, f);
                }

                const BITS_PER_DIGIT: usize = <$base>::BITS_PER_DIGIT;
                let alphabet: &[u8; 16] = if $upper {
                    b"0123456789ABCDEF"
                } else {
                    b"0123456789abcdef"
                };
                let mask: u64 = (1 << BITS_PER_DIGIT) - 1;

                let bit_len = self.bit_len();
                let total_digits = bit_len.div_ceil(BITS_PER_DIGIT);

                let mut s = StackString::<BITS>::new();
                let mut i = total_digits;
                while i > 0 {
                    i -= 1;
                    let bit_offset = i * BITS_PER_DIGIT;
                    let limb_idx = bit_offset / 64;
                    let bit_idx = bit_offset % 64;
                    let mut digit = (self.limbs[limb_idx] >> bit_idx) & mask;
                    if bit_idx + BITS_PER_DIGIT > 64 && limb_idx + 1 < LIMBS {
                        digit |= (self.limbs[limb_idx + 1] << (64 - bit_idx)) & mask;
                    }
                    s.push_byte(alphabet[digit as usize]);
                }
                f.pad_integral(true, <$base>::PREFIX, s.as_str())
            }
        }
    };
}

impl<const BITS: usize, const LIMBS: usize> fmt::Debug for Uint<BITS, LIMBS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<const BITS: usize, const LIMBS: usize> fmt::Display for Uint<BITS, LIMBS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if BITS == 0 {
            return f.pad_integral(true, base::Decimal::PREFIX, "0");
        }

        let mut buffer = StackString::<BITS>::new();
        self.write_decimal(&mut buffer);
        f.pad_integral(true, base::Decimal::PREFIX, buffer.as_str())
    }
}

impl_fmt_pow2!(fmt::Binary; base::Binary, false);
impl_fmt_pow2!(fmt::Octal; base::Octal, false);
impl_fmt_pow2!(fmt::LowerHex; base::Hexadecimal, false);
impl_fmt_pow2!(fmt::UpperHex; base::Hexadecimal, true);

impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    fn write_decimal(&self, buffer: &mut impl DecimalBuffer) {
        let mut spigots = self.to_base_be_2(base::Decimal::MAX);
        let Some(first) = spigots.next() else {
            buffer.push_byte(b'0');
            return;
        };

        push_decimal(buffer, first, 0);
        for spigot in spigots {
            push_decimal(buffer, spigot, base::Decimal::WIDTH);
        }
    }
}

#[cfg(feature = "alloc")]
impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    /// Converts this integer to a decimal string.
    ///
    /// This method intentionally shadows [`ToString::to_string`].
    #[allow(clippy::inherent_to_string_shadow_display)]
    #[inline]
    #[must_use]
    pub fn to_string(&self) -> String {
        let mut buffer = Vec::with_capacity(const { decimal_capacity(BITS) });
        self.write_decimal(&mut buffer);
        // SAFETY: `write_decimal` only writes ASCII decimal digits.
        unsafe { String::from_utf8_unchecked(buffer) }
    }
}

trait DecimalBuffer {
    fn len(&self) -> usize;
    fn push_byte(&mut self, byte: u8);
    fn set_byte(&mut self, index: usize, byte: u8);
}

#[cfg(feature = "alloc")]
impl DecimalBuffer for Vec<u8> {
    #[inline]
    fn len(&self) -> usize {
        Vec::len(self)
    }

    #[inline]
    fn push_byte(&mut self, byte: u8) {
        self.push(byte);
    }

    #[inline]
    fn set_byte(&mut self, index: usize, byte: u8) {
        self[index] = byte;
    }
}

fn push_decimal(buffer: &mut impl DecimalBuffer, mut value: u64, min_width: usize) {
    let digits = if value == 0 {
        1
    } else {
        value.ilog10() as usize + 1
    };
    let width = digits.max(min_width);
    let start = buffer.len();
    for _ in 0..width {
        buffer.push_byte(b'0');
    }

    let mut i = start + width;
    loop {
        i -= 1;
        buffer.set_byte(i, b'0' + (value % 10) as u8);
        value /= 10;
        if value == 0 {
            break;
        }
    }
}

/// A stack-allocated buffer that implements [`fmt::Write`].
pub(crate) struct StackString<const SIZE: usize> {
    len: usize,
    buf: [MaybeUninit<u8>; SIZE],
}

impl<const SIZE: usize> StackString<SIZE> {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            len: 0,
            buf: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    #[inline]
    pub(crate) const fn as_str(&self) -> &str {
        // SAFETY: `buf` is only written with valid UTF-8 by `fmt::Write` or
        // ASCII bytes by `push_byte` and `DecimalBuffer`.
        unsafe { core::str::from_utf8_unchecked(self.as_bytes()) }
    }

    #[inline]
    const fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.buf.as_ptr().cast(), self.len) }
    }

    #[inline]
    fn push_byte(&mut self, b: u8) {
        debug_assert!(self.len < SIZE);
        unsafe { self.buf.as_mut_ptr().add(self.len).cast::<u8>().write(b) };
        self.len += 1;
    }
}

impl<const SIZE: usize> DecimalBuffer for StackString<SIZE> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }

    #[inline]
    fn push_byte(&mut self, byte: u8) {
        StackString::push_byte(self, byte);
    }

    #[inline]
    fn set_byte(&mut self, index: usize, byte: u8) {
        debug_assert!(index < self.len);
        unsafe { self.buf.as_mut_ptr().add(index).cast::<u8>().write(byte) };
    }
}

impl<const SIZE: usize> fmt::Write for StackString<SIZE> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.len + s.len() > SIZE {
            return Err(fmt::Error);
        }
        unsafe {
            let dst = self.buf.as_mut_ptr().add(self.len).cast();
            core::ptr::copy_nonoverlapping(s.as_ptr(), dst, s.len());
        }
        self.len += s.len();
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        let clen = c.len_utf8();
        if self.len + clen > SIZE {
            return Err(fmt::Error);
        }
        c.encode_utf8(unsafe {
            core::slice::from_raw_parts_mut(self.buf.as_mut_ptr().add(self.len).cast(), clen)
        });
        self.len += clen;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{prop_assert_eq, proptest};

    #[allow(clippy::unreadable_literal)]
    const N: Uint<256, 4> = Uint::from_limbs([
        0xa8ec92344438aaf4_u64,
        0x9819ebdbd1faaab1_u64,
        0x573b1a7064c19c1a_u64,
        0xc85ef7d79691fe79_u64,
    ]);

    #[test]
    fn test_num() {
        assert_eq!(
            N.to_string(),
            "90630363884335538722706632492458228784305343302099024356772372330524102404852"
        );
        assert_eq!(
            format!("{N:x}"),
            "c85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4"
        );
        assert_eq!(
            format!("{N:b}"),
            "1100100001011110111101111101011110010110100100011111111001111001010101110011101100011010011100000110010011000001100111000001101010011000000110011110101111011011110100011111101010101010101100011010100011101100100100100011010001000100001110001010101011110100"
        );
        assert_eq!(
            format!("{N:o}"),
            "14413675753626443771712563543234062301470152300636573364375252543243544443210416125364"
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_to_string() {
        let string = Uint::<4096, 64>::ZERO.to_string();
        assert_eq!(string, "0");
        assert!(string.capacity() >= 1234);

        let zero = Uint::<0, 0>::ZERO.to_string();
        assert_eq!(zero, "0");
        assert!(zero.capacity() >= 1);

        assert_eq!(decimal_capacity(0), 1);
        assert_eq!(decimal_capacity(64), 20);
        assert_eq!(decimal_capacity(256), 78);
        assert_eq!(decimal_capacity(4096), 1234);

        proptest!(|(value: u128)| {
            let n = Uint::<128, 2>::from(value);
            prop_assert_eq!(n.to_string(), alloc::string::ToString::to_string(&value));
        });
    }

    #[test]
    fn test_fmt() {
        proptest!(|(value: u128)| {
            let n: Uint<128, 2> = Uint::from(value);

            prop_assert_eq!(format!("{n:b}"), format!("{value:b}"));
            prop_assert_eq!(format!("{n:064b}"), format!("{value:064b}"));
            prop_assert_eq!(format!("{n:#b}"), format!("{value:#b}"));

            prop_assert_eq!(format!("{n:o}"), format!("{value:o}"));
            prop_assert_eq!(format!("{n:064o}"), format!("{value:064o}"));
            prop_assert_eq!(format!("{n:#o}"), format!("{value:#o}"));

            prop_assert_eq!(format!("{n:}"), format!("{value:}"));
            prop_assert_eq!(format!("{n:064}"), format!("{value:064}"));
            prop_assert_eq!(format!("{n:#}"), format!("{value:#}"));
            prop_assert_eq!(format!("{n:?}"), format!("{value:?}"));
            prop_assert_eq!(format!("{n:064}"), format!("{value:064?}"));
            prop_assert_eq!(format!("{n:#?}"), format!("{value:#?}"));

            prop_assert_eq!(format!("{n:x}"), format!("{value:x}"));
            prop_assert_eq!(format!("{n:064x}"), format!("{value:064x}"));
            prop_assert_eq!(format!("{n:#x}"), format!("{value:#x}"));

            prop_assert_eq!(format!("{n:X}"), format!("{value:X}"));
            prop_assert_eq!(format!("{n:064X}"), format!("{value:064X}"));
            prop_assert_eq!(format!("{n:#X}"), format!("{value:#X}"));
        });
    }
}
