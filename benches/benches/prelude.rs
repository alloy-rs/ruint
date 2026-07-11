#![allow(unused_imports)]
#![allow(clippy::incompatible_msrv)]

use std::cell::RefCell;

pub use criterion::{BatchSize, Criterion};
pub use proptest::{
    arbitrary::Arbitrary,
    num::sample_uniform_incl,
    strategy::{Strategy, ValueTree},
    test_runner::TestRunner,
};
pub use ruint::{Bits, Uint, UintTryFrom, UintTryTo, const_for, nlimbs, uint};
pub use std::hint::black_box;

pub fn bench_unop<const BITS: usize, const LIMBS: usize, U>(
    criterion: &mut criterion::Criterion,
    name: &str,
    f: impl FnMut(Uint<BITS, LIMBS>) -> U,
) {
    bench_arbitrary::<Uint<BITS, LIMBS>, _>(criterion, &format!("{name}/{BITS}"), f);
}

pub fn bench_binop<const BITS: usize, const LIMBS: usize, U>(
    criterion: &mut criterion::Criterion,
    name: &str,
    mut f: impl FnMut(Uint<BITS, LIMBS>, Uint<BITS, LIMBS>) -> U,
) {
    bench_arbitrary::<(Uint<BITS, LIMBS>, Uint<BITS, LIMBS>), _>(
        criterion,
        &format!("{name}/{BITS}"),
        move |(a, b)| f(a, b),
    );
}

pub fn bench_ternary<const BITS: usize, const LIMBS: usize, U>(
    criterion: &mut criterion::Criterion,
    name: &str,
    mut f: impl FnMut(Uint<BITS, LIMBS>, Uint<BITS, LIMBS>, Uint<BITS, LIMBS>) -> U,
) {
    bench_arbitrary::<(Uint<BITS, LIMBS>, Uint<BITS, LIMBS>, Uint<BITS, LIMBS>), _>(
        criterion,
        &format!("{name}/{BITS}"),
        move |(a, b, c)| f(a, b, c),
    );
}

pub fn bench_arbitrary<T: Arbitrary, U>(
    criterion: &mut criterion::Criterion,
    name: &str,
    f: impl FnMut(<T::Strategy as proptest::strategy::Strategy>::Value) -> U,
) {
    bench_arbitrary_with(criterion, name, T::arbitrary(), f)
}

pub fn bench_arbitrary_with<T: Strategy, U>(
    criterion: &mut criterion::Criterion,
    name: &str,
    input: T,
    f: impl FnMut(T::Value) -> U,
) {
    let runner = std::cell::RefCell::new(TestRunner::deterministic());
    let mut setup = mk_setup(&input, &runner);
    let setup2 = mk_setup(&input, &runner);
    let mut f = manual_batch(setup2, f, name);
    criterion.bench_function(name, move |bencher| {
        bencher.iter_batched(&mut setup, &mut f, BatchSize::SmallInput);
    });
}

fn mk_setup<'a, T: Strategy>(
    input: &'a T,
    runner: &'a RefCell<TestRunner>,
) -> impl FnMut() -> T::Value + 'a {
    move || input.new_tree(&mut runner.borrow_mut()).unwrap().current()
}

/// Codspeed does not batch inputs even if `iter_batched` is used, so we have to
/// do it ourselves for operations that would otherwise be too fast to be
/// measured accurately.
#[cfg(codspeed)]
#[inline]
fn manual_batch<T, U>(
    mut setup: impl FnMut() -> T,
    mut f: impl FnMut(T) -> U,
    name: &str,
) -> impl FnMut(T) {
    assert!(
        !needs_drop::<T>(),
        "cannot batch inputs that need to be dropped: {}",
        std::any::type_name::<T>(),
    );
    assert!(
        !needs_drop::<U>(),
        "cannot batch outputs that need to be dropped: {}",
        std::any::type_name::<U>(),
    );

    let batch_size = black_box(get_batch_size(name));
    let inputs = black_box((0..batch_size).map(|_| setup()).collect::<Box<[_]>>());
    let mut out = black_box(Box::new_uninit_slice(batch_size));
    move |_| {
        for i in 0..batch_size {
            let input = unsafe { std::ptr::read(inputs.get_unchecked(i)) };
            let output = unsafe { out.get_unchecked_mut(i) };
            output.write(f(input));
        }
    }
}

#[cfg(codspeed)]
fn needs_drop<T>() -> bool {
    // SAFETY: `ArrayVec` doesn't implement `Copy` when `T: Copy` even though it
    // can.
    if std::any::type_name::<T>().contains("ArrayVec<u64") {
        false
    } else {
        std::mem::needs_drop::<T>()
    }
}

#[cfg(not(codspeed))]
fn manual_batch<T, U>(
    _setup: impl FnMut() -> T,
    f: impl FnMut(T) -> U,
    _name: &str,
) -> impl FnMut(T) -> U {
    f
}

#[allow(dead_code)]
fn get_batch_size(name: &str) -> usize {
    // Chained and composite-kernel benches do 16-64 core ops per call; batch less.
    if name.starts_with("chained/") || name.starts_with("curves/") {
        return 100;
    }
    let size = name.split('/').flat_map(str::parse::<usize>).max();
    if name.contains("pow") {
        if size >= Some(4096) { 1 } else { 100 }
    } else if size >= Some(4096) {
        100
    } else {
        10000
    }
}
