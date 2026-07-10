//! Composite elliptic-curve kernels built from ruint field primitives:
//! Jacobian point double/add (secp256k1 and BLS12-381 G1, both a = 0),
//! Fp2 multiplication, a 16-bit double-and-add scalar segment, the ECDSA
//! verify scalar prologue, and batch inversion. These exercise the register
//! pressure, op mix, and memory patterns of real signature/pairing code,
//! which single-op benches cannot.

use super::fields::{check_field, Fq, BLS12_381_P, SECP256K1_P, U256};
use crate::prelude::*;

const G_X: U256 = uint!(0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798_U256);
const G_Y: U256 = uint!(0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8_U256);
const G2_X: U256 = uint!(0xc6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5_U256);
const G2_Y: U256 = uint!(0x1ae168fea63dc339a3c58419466ceaeef7f632653266d0e1236431a950cfe52a_U256);
const G3_X: U256 = uint!(0xf9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9_U256);
const G3_Y: U256 = uint!(0x388f7b0f632de8140fe337e62a37f3566500a99934c2231b6cb9fd7584b8e672_U256);

/// Jacobian point (x/z², y/z³), coordinates in Montgomery form.
#[derive(Clone, Copy, Debug)]
struct Jac<const BITS: usize, const LIMBS: usize> {
    x: Uint<BITS, LIMBS>,
    y: Uint<BITS, LIMBS>,
    z: Uint<BITS, LIMBS>,
}

/// Doubling for a = 0 curves (EFD dbl-2009-l).
fn jac_double<const BITS: usize, const LIMBS: usize>(
    f: &Fq<BITS, LIMBS>,
    p: Jac<BITS, LIMBS>,
) -> Jac<BITS, LIMBS> {
    let a = f.sqr(p.x);
    let b = f.sqr(p.y);
    let c = f.sqr(b);
    let d = {
        let t = f.sub(f.sub(f.sqr(f.add(p.x, b)), a), c);
        f.add(t, t)
    };
    let e = f.add(f.add(a, a), a);
    let ff = f.sqr(e);
    let x3 = f.sub(ff, f.add(d, d));
    let c8 = {
        let c2 = f.add(c, c);
        let c4 = f.add(c2, c2);
        f.add(c4, c4)
    };
    let y3 = f.sub(f.mul(e, f.sub(d, x3)), c8);
    let z3 = {
        let t = f.mul(p.y, p.z);
        f.add(t, t)
    };
    Jac { x: x3, y: y3, z: z3 }
}

/// General addition (EFD add-2007-bl). Requires P ≠ ±Q and both nonzero.
fn jac_add<const BITS: usize, const LIMBS: usize>(
    f: &Fq<BITS, LIMBS>,
    p: Jac<BITS, LIMBS>,
    q: Jac<BITS, LIMBS>,
) -> Jac<BITS, LIMBS> {
    let z1z1 = f.sqr(p.z);
    let z2z2 = f.sqr(q.z);
    let u1 = f.mul(p.x, z2z2);
    let u2 = f.mul(q.x, z1z1);
    let s1 = f.mul(f.mul(p.y, q.z), z2z2);
    let s2 = f.mul(f.mul(q.y, p.z), z1z1);
    let h = f.sub(u2, u1);
    let i = {
        let h2 = f.add(h, h);
        f.sqr(h2)
    };
    let j = f.mul(h, i);
    let r = {
        let t = f.sub(s2, s1);
        f.add(t, t)
    };
    let v = f.mul(u1, i);
    let x3 = f.sub(f.sub(f.sqr(r), j), f.add(v, v));
    let y3 = {
        let t = f.mul(s1, j);
        f.sub(f.mul(r, f.sub(v, x3)), f.add(t, t))
    };
    let z3 = f.mul(f.sub(f.sub(f.sqr(f.add(p.z, q.z)), z1z1), z2z2), h);
    Jac { x: x3, y: y3, z: z3 }
}

/// Montgomery-form Jacobian point back to plain affine coordinates.
fn to_affine<const BITS: usize, const LIMBS: usize>(
    f: &Fq<BITS, LIMBS>,
    p: Jac<BITS, LIMBS>,
) -> (Uint<BITS, LIMBS>, Uint<BITS, LIMBS>) {
    let (x, y, z) = (f.from_mont(p.x), f.from_mont(p.y), f.from_mont(p.z));
    let zi = z.inv_mod(f.p).unwrap();
    let zi2 = zi.mul_mod(zi, f.p);
    (x.mul_mod(zi2, f.p), y.mul_mod(zi2.mul_mod(zi, f.p), f.p))
}

/// Known-answer checks: a wrong formula or constant panics here.
fn sanity_checks() {
    let f = check_field("secp256k1_p", SECP256K1_P);
    let g = Jac { x: f.to_mont(G_X), y: f.to_mont(G_Y), z: f.r1 };
    let g2 = jac_double(&f, g);
    assert_eq!(to_affine(&f, g2), (G2_X, G2_Y), "jac_double: 2G mismatch");
    let g3 = jac_add(&f, g2, g);
    assert_eq!(to_affine(&f, g3), (G3_X, G3_Y), "jac_add: 3G mismatch");
}

fn jac_point_strategy<const BITS: usize, const LIMBS: usize>(
    p: Uint<BITS, LIMBS>,
) -> impl Strategy<Value = Jac<BITS, LIMBS>> {
    <(Uint<BITS, LIMBS>, Uint<BITS, LIMBS>, Uint<BITS, LIMBS>)>::arbitrary().prop_map(
        move |(x, y, z)| Jac { x: x.reduce_mod(p), y: y.reduce_mod(p), z: z.reduce_mod(p) },
    )
}

fn bench_curve<const BITS: usize, const LIMBS: usize>(
    criterion: &mut Criterion,
    name: &str,
    p: Uint<BITS, LIMBS>,
) {
    let f = Fq::new(p);
    bench_arbitrary_with(
        criterion,
        &format!("curves/{name}/jac_double"),
        jac_point_strategy(p),
        move |pt| jac_double(&f, pt),
    );
    bench_arbitrary_with(
        criterion,
        &format!("curves/{name}/jac_add"),
        (jac_point_strategy(p), jac_point_strategy(p)),
        move |(a, b)| jac_add(&f, a, b),
    );
}

pub fn group(criterion: &mut Criterion) {
    sanity_checks();
    bench_curve(criterion, "secp256k1", SECP256K1_P);
    bench_curve(criterion, "bls12_381_g1", BLS12_381_P);
}
