use crate::prelude::*;

pub fn group(criterion: &mut Criterion) {
    const_for!(BITS in BENCH {
        const LIMBS: usize = nlimbs(BITS);

        let mut buf = Vec::with_capacity(2 + 2 + BITS / 4);
        bench_unop::<BITS, LIMBS, _>(criterion, "serde/json", |n| {
            buf.clear();
            serde_json::to_writer(black_box(&mut buf), &n).unwrap()
        });

        // Values that fit in a single limb, as most quantities do.
        let mut buf = Vec::with_capacity(2 + 2 + BITS / 4);
        bench_arbitrary::<u64, _>(criterion, &format!("serde/json/small/{BITS}"), |n| {
            buf.clear();
            serde_json::to_writer(black_box(&mut buf), &Uint::<BITS, LIMBS>::from(n)).unwrap()
        });
    });
}
