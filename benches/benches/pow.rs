use crate::prelude::*;

pub fn group(criterion: &mut Criterion) {
    const_for!(BITS in BENCH {
        const LIMBS: usize = nlimbs(BITS);
        type U = Uint<BITS, LIMBS>;
        bench_binop::<BITS, LIMBS, _>(criterion, "pow", |a, b| a.pow(b));
        bench_binop::<BITS, LIMBS, _>(criterion, "overflowing_pow", |a, b| a.overflowing_pow(b));

        // Full-width exponents with an odd and an even base.
        let name = format!("pow_odd/{BITS}");
        bench_arbitrary_with(
            criterion,
            &name,
            (U::arbitrary(), U::arbitrary()).prop_map(|(a, b)| (a | U::ONE, b)),
            |(a, b)| a.pow(b),
        );
        let name = format!("pow_even/{BITS}");
        bench_arbitrary_with(
            criterion,
            &name,
            (U::arbitrary(), U::arbitrary()).prop_map(|(a, b)| (a.wrapping_shl(1), b)),
            |(a, b)| a.pow(b),
        );

        // Small exponents, like `10 ** decimals`.
        let name = format!("pow_small_exp/{BITS}");
        bench_arbitrary_with(
            criterion,
            &name,
            (3_u64..1000, 0_u64..20).prop_map(|(a, b)| (U::wrapping_from(a), U::wrapping_from(b))),
            |(a, b)| a.pow(b),
        );
        let name = format!("pow_10/{BITS}");
        bench_arbitrary_with(
            criterion,
            &name,
            (0_u64..78).prop_map(|b| (U::wrapping_from(10), U::wrapping_from(b))),
            |(a, b)| a.pow(b),
        );
    });
}
