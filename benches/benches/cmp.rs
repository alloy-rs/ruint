use crate::prelude::*;

pub fn group(criterion: &mut Criterion) {
    const_for!(BITS in BENCH {
        const LIMBS: usize = nlimbs(BITS);
        bench_unop::<BITS, LIMBS, _>(criterion, "is_zero", |a| a.is_zero());
        bench_unop::<BITS, LIMBS, _>(criterion, "const_is_zero", |a| a.const_is_zero());
        bench_binop::<BITS, LIMBS, _>(criterion, "eq", |a, b| a == b);
        bench_binop::<BITS, LIMBS, _>(criterion, "const_eq", |a, b| a.const_eq(&b));
        bench_binop::<BITS, LIMBS, _>(criterion, "cmp", |a, b| a.cmp(&b));
        bench_binop::<BITS, LIMBS, _>(criterion, "lt", |a, b| a < b);
        bench_binop::<BITS, LIMBS, _>(criterion, "gt", |a, b| a > b);
        bench_binop::<BITS, LIMBS, _>(criterion, "le", |a, b| a <= b);
        bench_binop::<BITS, LIMBS, _>(criterion, "ge", |a, b| a >= b);
        bench_binop::<BITS, LIMBS, _>(criterion, "min", |a, b| a.min(b));
        bench_binop::<BITS, LIMBS, _>(criterion, "max", |a, b| a.max(b));
    });
}
