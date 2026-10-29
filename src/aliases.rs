//! Type aliases for common bit sizes of [`Uint`], [`Int`] and [`Bits`].
use crate::{Bits, Int, Uint};

/// [`Uint`] for `0` bits. Always zero. Similar to `()`.
pub type U0 = Uint<0, 0>;

/// [`Uint`] for `1` bit. Similar to [`bool`].
pub type U1 = Uint<1, 1>;

/// [`Uint`] for `8` bits. Similar to [`u8`].
pub type U8 = Uint<8, 1>;

/// [`Uint`] for `16` bits. Similar to [`u16`].
pub type U16 = Uint<16, 1>;

/// [`Uint`] for `32` bits. Similar to [`u32`].
pub type U32 = Uint<32, 1>;

/// [`Uint`] for `64` bits. Similar to [`u64`].
pub type U64 = Uint<64, 1>;

/// [`Uint`] for `128` bits. Similar to [`u128`].
pub type U128 = Uint<128, 2>;

/// [`Int`] for `0` bits. Always zero. Similar to `()`.
pub type I0 = Int<0, 0>;

/// [`Int`] for `1` bit. Either zero or minus one.
pub type I1 = Int<1, 1>;

/// [`Int`] for `8` bits. Similar to [`i8`].
pub type I8 = Int<8, 1>;

/// [`Int`] for `16` bits. Similar to [`i16`].
pub type I16 = Int<16, 1>;

/// [`Int`] for `32` bits. Similar to [`i32`].
pub type I32 = Int<32, 1>;

/// [`Int`] for `64` bits. Similar to [`i64`].
pub type I64 = Int<64, 1>;

/// [`Int`] for `128` bits. Similar to [`i128`].
pub type I128 = Int<128, 2>;

macro_rules! bit_alias {
    ($($name:ident($bits:expr, $limbs:expr);)*) => {$(
        #[doc = concat!("[`Bits`] for `", stringify!($bits),"` bits.")]
        pub type $name = Bits<$bits, $limbs>;
    )*};
}

bit_alias! {
    B0(0, 0);
    B1(1, 1);
    B8(8, 1);
    B16(16, 1);
    B32(32, 1);
    B64(64, 1);
    B128(128, 2);
}

macro_rules! alias {
    ($($uname:ident $iname:ident $bname:ident ($bits:expr, $limbs:expr);)*) => {$(
        #[doc = concat!("[`Uint`] for `", stringify!($bits),"` bits.")]
        pub type $uname = Uint<$bits, $limbs>;
        #[doc = concat!("[`Int`] for `", stringify!($bits),"` bits.")]
        pub type $iname = Int<$bits, $limbs>;
        #[doc = concat!("[`Bits`] for `", stringify!($bits),"` bits.")]
        pub type $bname = Bits<$bits, $limbs>;
    )*};
}

alias! {
    U160 I160 B160 (160, 3);
    U192 I192 B192 (192, 3);
    U256 I256 B256 (256, 4);
    U320 I320 B320 (320, 5);
    U384 I384 B384 (384, 6);
    U448 I448 B448 (448, 7);
    U512 I512 B512 (512, 8);
    U768 I768 B768 (768, 12);
    U1024 I1024 B1024 (1024, 16);
    U2048 I2048 B2048 (2048, 32);
    U4096 I4096 B4096 (4096, 64);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    const fn instantiate_consts() {
        let _ = (U0::ZERO, U0::MAX, B0::ZERO);
        let _ = (I0::ZERO, I0::MIN, I0::MAX);
        let _ = (U1::ZERO, U1::MAX, B1::ZERO);
        let _ = (I1::ZERO, I1::MIN, I1::MAX);
        let _ = (U8::ZERO, U8::MAX, B8::ZERO);
        let _ = (I8::ZERO, I8::MIN, I8::MAX);
        let _ = (U16::ZERO, U16::MAX, B16::ZERO);
        let _ = (I16::ZERO, I16::MIN, I16::MAX);
        let _ = (U32::ZERO, U32::MAX, B32::ZERO);
        let _ = (I32::ZERO, I32::MIN, I32::MAX);
        let _ = (U64::ZERO, U64::MAX, B64::ZERO);
        let _ = (I64::ZERO, I64::MIN, I64::MAX);
        let _ = (U128::ZERO, U128::MAX, B128::ZERO);
        let _ = (I128::ZERO, I128::MIN, I128::MAX);
        let _ = (U160::ZERO, U160::MAX, B160::ZERO);
        let _ = (I160::ZERO, I160::MIN, I160::MAX);
        let _ = (U192::ZERO, U192::MAX, B192::ZERO);
        let _ = (I192::ZERO, I192::MIN, I192::MAX);
        let _ = (U256::ZERO, U256::MAX, B256::ZERO);
        let _ = (I256::ZERO, I256::MIN, I256::MAX);
        let _ = (U320::ZERO, U320::MAX, B320::ZERO);
        let _ = (I320::ZERO, I320::MIN, I320::MAX);
        let _ = (U384::ZERO, U384::MAX, B384::ZERO);
        let _ = (I384::ZERO, I384::MIN, I384::MAX);
        let _ = (U448::ZERO, U448::MAX, B448::ZERO);
        let _ = (I448::ZERO, I448::MIN, I448::MAX);
        let _ = (U512::ZERO, U512::MAX, B512::ZERO);
        let _ = (I512::ZERO, I512::MIN, I512::MAX);
        let _ = (U768::ZERO, U768::MAX, B768::ZERO);
        let _ = (I768::ZERO, I768::MIN, I768::MAX);
        let _ = (U1024::ZERO, U1024::MAX, B1024::ZERO);
        let _ = (I1024::ZERO, I1024::MIN, I1024::MAX);
        let _ = (U2048::ZERO, U2048::MAX, B2048::ZERO);
        let _ = (I2048::ZERO, I2048::MIN, I2048::MAX);
        let _ = (U4096::ZERO, U4096::MAX, B4096::ZERO);
        let _ = (I4096::ZERO, I4096::MIN, I4096::MAX);
    }
}
