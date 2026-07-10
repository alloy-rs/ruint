//! Comparison benchmarks with *realistic* input distributions. The existing
//! `cmp` group uses uniform-random inputs, where `eq` is decided in the first
//! limb ~100% of the time and every branch is perfectly predictable. Real
//! 256-bit words are skewed: small counters, 160-bit addresses, hashes, and
//! equality checks that are usually true or differ late. Branchy and
//! branchless implementations rank differently across these distributions —
//! wall-clock only; CodSpeed's simulator does not model prediction.

use super::fields::U256;
use crate::prelude::*;
use proptest::{prop_oneof, strategy::Just};

pub fn group(criterion: &mut Criterion) {
    // eq: outcome and differing-limb position.
    bench_arbitrary_with(
        criterion,
        "dist/eq/equal/256",
        U256::arbitrary().prop_map(|x| (x, x)),
        |(a, b)| a == b,
    );
    bench_arbitrary_with(
        criterion,
        "dist/eq/differ_low/256",
        U256::arbitrary().prop_map(|x| (x, x ^ U256::ONE)),
        |(a, b)| a == b,
    );
    bench_arbitrary_with(
        criterion,
        "dist/eq/differ_high/256",
        U256::arbitrary().prop_map(|x| (x, x ^ (U256::ONE << 255))),
        |(a, b)| a == b,
    );
    // 50/50 equal: hostile to branch prediction, friendly to branchless.
    bench_arbitrary_with(
        criterion,
        "dist/eq/mixed50/256",
        <(U256, bool)>::arbitrary().prop_map(|(x, e)| if e { (x, x) } else { (x, x ^ U256::ONE) }),
        |(a, b)| a == b,
    );

    // is_zero: mostly-nonzero stream with 10% zeros (flag-checking pattern).
    bench_arbitrary_with(
        criterion,
        "dist/is_zero/mixed10/256",
        prop_oneof![
            9 => U256::arbitrary().prop_map(|x| x | U256::ONE),
            1 => Just(U256::ZERO),
        ],
        |a| a.is_zero(),
    );

    // lt: magnitude distributions.
    let small = || <(u64, u64)>::arbitrary().prop_map(|(a, b)| (U256::from(a), U256::from(b)));
    bench_arbitrary_with(criterion, "dist/lt/small64/256", small(), |(a, b)| a < b);
    let addr_mask: U256 = (U256::ONE << 160) - U256::ONE;
    bench_arbitrary_with(
        criterion,
        "dist/lt/address160/256",
        <(U256, U256)>::arbitrary().prop_map(move |(a, b)| (a & addr_mask, b & addr_mask)),
        |(a, b)| a < b,
    );
    bench_arbitrary_with(
        criterion,
        "dist/lt/mixed_magnitude/256",
        <(u64, U256)>::arbitrary().prop_map(|(a, b)| (U256::from(a), b)),
        |(a, b)| a < b,
    );

    // lt feeding divergent arms: comparison result steers real work.
    bench_arbitrary_with(criterion, "dist/lt_arms/small64/256", small(), |(a, b)| {
        if a < b {
            a.wrapping_add(b)
        } else {
            a.wrapping_sub(b)
        }
    });
    bench_arbitrary_with(
        criterion,
        "dist/lt_arms/random/256",
        <(U256, U256)>::arbitrary(),
        |(a, b)| {
            if a < b {
                a.wrapping_add(b)
            } else {
                a.wrapping_sub(b)
            }
        },
    );
}
