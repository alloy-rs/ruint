//! Field-arithmetic kernels over the fixed moduli that dominate real-world
//! ruint usage: secp256k1 (k256 ECDSA, both the base field and the scalar
//! order), NIST P-256, BN254, BLS12-381, BLS12-377, and CSIDH-512 as the
//! post-quantum representative.
//!
//! Unlike the generic `modular` group (random moduli), these benches use the
//! actual primes, real fixed exponents (Fermat inversion, sqrt for point
//! decompression, 65537), and Montgomery-domain kernels (`mul_redc`,
//! `square_redc`) — the true inner loops of signature and pairing libraries.

use crate::prelude::*;
use ruint::aliases::U64;

pub(crate) type U256 = Uint<256, 4>;
pub(crate) type U384 = Uint<384, 6>;
pub(crate) type U512 = Uint<512, 8>;

// Moduli verified against their generating formulas:
// secp256k1 p = 2^256 − 2^32 − 977; P-256 p = 2^256 − 2^224 + 2^192 + 2^96 − 1;
// BN254 from t = 4965661367192848881; BLS12-381 from z = −0xd201000000010000;
// BLS12-377 from z = 0x8508c00000000001;
// CSIDH-512 p = 4·(∏ first 73 odd primes)·587 − 1.
pub(crate) const SECP256K1_P: U256 =
    uint!(0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f_U256);
pub(crate) const SECP256K1_N: U256 =
    uint!(0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141_U256);
pub(crate) const P256_P: U256 =
    uint!(0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff_U256);
pub(crate) const P256_N: U256 =
    uint!(0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551_U256);
pub(crate) const BN254_P: U256 =
    uint!(0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47_U256);
pub(crate) const BN254_R: U256 =
    uint!(0x30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001_U256);
pub(crate) const BLS12_381_P: U384 =
    uint!(0x1a0111ea397fe69a4b1ba7b6434bacd764774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaab_U384);
pub(crate) const BLS12_381_R: U256 =
    uint!(0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001_U256);
pub(crate) const BLS12_377_P: U384 =
    uint!(0x1ae3a4617c510eac63b05c06ca1493b1a22d9f300f5138f1ef3622fba094800170b5d44300000008508c00000000001_U384);
pub(crate) const BLS12_377_R: U256 =
    uint!(0x12ab655e9a2ca55660b44d1e5c37b00159aa76fed00000010a11800000000001_U256);
pub(crate) const CSIDH512_P: U512 =
    uint!(0x65b48e8f740f89bffc8ab0d15e3e4c4ab42d083aedc88c425afbfcc69322c9cda7aac6c567f35507516730cc1f0b4f25c2721bf457aca8351b81b90533c6c87b_U512);

/// Montgomery context for a fixed odd prime modulus.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fq<const BITS: usize, const LIMBS: usize> {
    pub p: Uint<BITS, LIMBS>,
    pub inv: u64,
    /// R mod p, i.e. the Montgomery form of 1.
    pub r1: Uint<BITS, LIMBS>,
    /// R² mod p.
    pub r2: Uint<BITS, LIMBS>,
}

impl<const BITS: usize, const LIMBS: usize> Fq<BITS, LIMBS> {
    pub fn new(p: Uint<BITS, LIMBS>) -> Self {
        let inv: u64 = U64::wrapping_from(p).inv_ring().unwrap().wrapping_neg().to();
        // R mod p = (2^BITS − 1 mod p) + 1; never wraps to 0 since p ∤ 2^BITS.
        let r1 = (Uint::MAX % p).wrapping_add(Uint::ONE);
        let r2 = r1.mul_mod(r1, p);
        Self { p, inv, r1, r2 }
    }

    pub fn mul(&self, a: Uint<BITS, LIMBS>, b: Uint<BITS, LIMBS>) -> Uint<BITS, LIMBS> {
        a.mul_redc(b, self.p, self.inv)
    }

    pub fn sqr(&self, a: Uint<BITS, LIMBS>) -> Uint<BITS, LIMBS> {
        a.square_redc(self.p, self.inv)
    }

    pub fn add(&self, a: Uint<BITS, LIMBS>, b: Uint<BITS, LIMBS>) -> Uint<BITS, LIMBS> {
        a.add_mod(b, self.p)
    }

    pub fn sub(&self, a: Uint<BITS, LIMBS>, b: Uint<BITS, LIMBS>) -> Uint<BITS, LIMBS> {
        // a − b mod p for a, b < p: the wrap of `wrapping_sub` and the +p cancel.
        let d = a.wrapping_sub(b);
        if a < b {
            d.wrapping_add(self.p)
        } else {
            d
        }
    }

    pub fn to_mont(&self, a: Uint<BITS, LIMBS>) -> Uint<BITS, LIMBS> {
        self.mul(a, self.r2)
    }

    pub fn from_mont(&self, a: Uint<BITS, LIMBS>) -> Uint<BITS, LIMBS> {
        self.mul(a, Uint::ONE)
    }

    /// Montgomery-domain inverse: maps xR to x⁻¹R.
    pub fn inv(&self, a: Uint<BITS, LIMBS>) -> Uint<BITS, LIMBS> {
        // inv_mod(xR) = x⁻¹R⁻¹; two redc-muls by R² append the two missing Rs.
        let i = a.inv_mod(self.p).unwrap();
        self.mul(self.mul(i, self.r2), self.r2)
    }
}

/// Build the context and verify it: wrong constants panic here instead of
/// silently benchmarking garbage.
pub(crate) fn check_field<const BITS: usize, const LIMBS: usize>(
    name: &str,
    p: Uint<BITS, LIMBS>,
) -> Fq<BITS, LIMBS> {
    assert!(p.bit(0), "{name}: modulus must be odd");
    let f = Fq::new(p);
    assert_eq!(f.inv.wrapping_mul(p.as_limbs()[0]), u64::MAX, "{name}: bad Montgomery inv");
    let a = p.wrapping_sub(Uint::from(5u64));
    let b = (p >> 1usize).wrapping_add(Uint::from(7u64));
    assert_eq!(f.from_mont(f.to_mont(a)), a, "{name}: Montgomery round-trip");
    assert_eq!(
        f.mul(f.to_mont(a), f.to_mont(b)),
        f.to_mont(a.mul_mod(b, p)),
        "{name}: mul_redc disagrees with mul_mod"
    );
    assert_eq!(f.sqr(f.to_mont(a)), f.mul(f.to_mont(a), f.to_mont(a)), "{name}: square_redc");
    assert_eq!(f.mul(f.inv(f.to_mont(b)), f.to_mont(b)), f.r1, "{name}: Montgomery inverse");
    f
}

fn bench_field<const BITS: usize, const LIMBS: usize>(
    criterion: &mut Criterion,
    name: &str,
    p: Uint<BITS, LIMBS>,
    sqrt: bool,
) {
    let f = check_field(name, p);
    let reduced = move || Uint::<BITS, LIMBS>::arbitrary().prop_map(move |x| x.reduce_mod(p));
    let pair = move || (reduced(), reduced());

    bench_arbitrary_with(criterion, &format!("fields/{name}/mul_redc"), pair(), move |(a, b)| {
        f.mul(a, b)
    });
    bench_arbitrary_with(criterion, &format!("fields/{name}/square_redc"), reduced(), move |a| {
        f.sqr(a)
    });
    bench_arbitrary_with(criterion, &format!("fields/{name}/add_mod"), pair(), move |(a, b)| {
        a.add_mod(b, p)
    });
    bench_arbitrary_with(criterion, &format!("fields/{name}/inv_mod"), reduced(), move |a| {
        a.inv_mod(p)
    });
    // Real fixed exponents: Fermat inversion a^(p−2), point-decompression sqrt
    // a^((p+1)/4) (p ≡ 3 mod 4 only), and the RSA verify exponent 65537.
    let fermat = p.wrapping_sub(Uint::from(2u64));
    bench_arbitrary_with(
        criterion,
        &format!("fields/{name}/pow_mod/fermat"),
        reduced(),
        move |a| a.pow_mod(fermat, p),
    );
    if sqrt {
        let e = (p >> 2usize).wrapping_add(Uint::ONE);
        bench_arbitrary_with(
            criterion,
            &format!("fields/{name}/pow_mod/sqrt"),
            reduced(),
            move |a| a.pow_mod(e, p),
        );
    }
    bench_arbitrary_with(
        criterion,
        &format!("fields/{name}/pow_mod/e65537"),
        reduced(),
        move |a| a.pow_mod(Uint::from(65537u64), p),
    );
}

pub fn group(criterion: &mut Criterion) {
    bench_field(criterion, "secp256k1_p", SECP256K1_P, true);
    bench_field(criterion, "secp256k1_n", SECP256K1_N, false);
    bench_field(criterion, "p256_p", P256_P, true);
    bench_field(criterion, "p256_n", P256_N, false);
    bench_field(criterion, "bn254_p", BN254_P, true);
    bench_field(criterion, "bn254_r", BN254_R, false);
    bench_field(criterion, "bls12_381_p", BLS12_381_P, true);
    bench_field(criterion, "bls12_381_r", BLS12_381_R, false);
    // BLS12-377 p ≡ 1 (mod 4): the (p+1)/4 sqrt exponent does not apply.
    bench_field(criterion, "bls12_377_p", BLS12_377_P, false);
    bench_field(criterion, "bls12_377_r", BLS12_377_R, false);
    bench_field(criterion, "csidh512_p", CSIDH512_P, true);
}
