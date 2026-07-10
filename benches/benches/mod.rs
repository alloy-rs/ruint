mod add;
mod algorithms;
mod base_convert;
mod bits;
mod cmp;
mod curves;
mod div;
mod fields;
mod fmt;
mod from;
mod latency;
mod log;
mod modular;
mod mul;
mod pow;
mod root;
mod string;

pub(crate) mod prelude;

pub fn group(c: &mut criterion::Criterion) {
    bits::group(c);

    add::group(c);
    mul::group(c);
    div::group(c);
    pow::group(c);
    log::group(c);
    root::group(c);
    modular::group(c);
    fields::group(c);
    latency::group(c);
    curves::group(c);

    cmp::group(c);

    base_convert::group(c);
    from::group(c);
    fmt::group(c);
    string::group(c);

    algorithms::group(c);
}
