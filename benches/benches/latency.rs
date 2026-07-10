//! Latency-chain benchmarks: each measurement is `CHAIN` applications of an
//! op where the output feeds the next input, so the CPU cannot overlap
//! iterations. This measures the *critical-path latency* of an op — the cost
//! that matters when its result feeds control or data flow (pow_mod ladders,
//! comparison-then-branch) — which throughput-style benches and CodSpeed's
//! instruction counting are both blind to.
//!
//! Reported time ≈ CHAIN × per-op latency; divide by `CHAIN` (64).

use super::fields::{BLS12_381_P, Fq, SECP256K1_P, U256};
use crate::prelude::*;

const CHAIN: usize = 64;

pub fn group(criterion: &mut Criterion) {
    // Plain wrapping ops. black_box pins the intermediate so the loop cannot
    // be reassociated or folded; it adds ~1 register move per link.
    bench_arbitrary_with(
        criterion,
        "chained/mul/256",
        <(U256, U256)>::arbitrary(),
        |(mut x, y)| {
            let y = y | U256::ONE;
            for _ in 0..CHAIN {
                x = black_box(x.wrapping_mul(y));
            }
            x
        },
    );
    bench_arbitrary_with(
        criterion,
        "chained/add/256",
        <(U256, U256)>::arbitrary(),
        |(mut x, y)| {
            for _ in 0..CHAIN {
                x = black_box(x.wrapping_add(y));
            }
            x
        },
    );

    // Comparison ops: the boolean result feeds the next value, modeling
    // compare-then-act code. The +1/+2 depends on the comparison, so the
    // dependency is enforced without black_box.
    chained_cmp::<256, 4>(criterion);
    chained_cmp::<512, 8>(criterion);

    // Montgomery kernels: x = f(x, y) is exactly the pow_mod inner loop.
    chained_redc(criterion, "secp256k1_p", SECP256K1_P);
    chained_redc(criterion, "bls12_381_p", BLS12_381_P);

    let p = SECP256K1_P;
    bench_arbitrary_with(
        criterion,
        "chained/add_mod/secp256k1_p",
        (reduced(p), reduced(p)),
        move |(mut x, y)| {
            for _ in 0..CHAIN {
                x = x.add_mod(y, p);
            }
            x
        },
    );
}

fn chained_cmp<const BITS: usize, const LIMBS: usize>(criterion: &mut Criterion) {
    type U<const BITS: usize, const LIMBS: usize> = Uint<BITS, LIMBS>;
    bench_arbitrary_with(
        criterion,
        &format!("chained/eq/{BITS}"),
        <(U<BITS, LIMBS>, U<BITS, LIMBS>)>::arbitrary(),
        |(mut x, y)| {
            for _ in 0..CHAIN {
                let e = (x == y) as u64;
                x = x.wrapping_add(U::from(e + 1));
            }
            x
        },
    );
    bench_arbitrary_with(
        criterion,
        &format!("chained/const_eq/{BITS}"),
        <(U<BITS, LIMBS>, U<BITS, LIMBS>)>::arbitrary(),
        |(mut x, y)| {
            for _ in 0..CHAIN {
                let e = x.const_eq(&y) as u64;
                x = x.wrapping_add(U::from(e + 1));
            }
            x
        },
    );
    bench_arbitrary_with(
        criterion,
        &format!("chained/is_zero/{BITS}"),
        U::<BITS, LIMBS>::arbitrary(),
        |mut x| {
            for _ in 0..CHAIN {
                let z = x.is_zero() as u64;
                x = x.wrapping_add(U::from(z + 1));
            }
            x
        },
    );
    bench_arbitrary_with(
        criterion,
        &format!("chained/const_is_zero/{BITS}"),
        U::<BITS, LIMBS>::arbitrary(),
        |mut x| {
            for _ in 0..CHAIN {
                let z = x.const_is_zero() as u64;
                x = x.wrapping_add(U::from(z + 1));
            }
            x
        },
    );
}

fn reduced<const BITS: usize, const LIMBS: usize>(
    p: Uint<BITS, LIMBS>,
) -> impl Strategy<Value = Uint<BITS, LIMBS>> {
    Uint::arbitrary().prop_map(move |x| x.reduce_mod(p))
}

fn chained_redc<const BITS: usize, const LIMBS: usize>(
    criterion: &mut Criterion,
    name: &str,
    p: Uint<BITS, LIMBS>,
) {
    let f = Fq::new(p);
    bench_arbitrary_with(
        criterion,
        &format!("chained/mul_redc/{name}"),
        (reduced(p), reduced(p)),
        move |(mut x, y)| {
            for _ in 0..CHAIN {
                x = f.mul(x, y);
            }
            x
        },
    );
    bench_arbitrary_with(
        criterion,
        &format!("chained/square_redc/{name}"),
        reduced(p),
        move |mut x| {
            for _ in 0..CHAIN {
                x = f.sqr(x);
            }
            x
        },
    );
}
