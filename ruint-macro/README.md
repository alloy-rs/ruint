# The `uint!` macro for `Uint`, `Int` and `Bits` literals

Within the [`uint!`] macro arguments, you can write [`Uint`], [`Int`] and [`Bits`] literals using the [same syntax][rust-syntax] as Rust integer literals, but using a capital `U`, `I` or `B` suffix respectively. Note that there is ambiguity for hexadecimals with a `B` suffix, to lessen the impact an underscore is required in this case.

Like Rust integer literals, `Int` literals have no sign: `-1_I256` negates `1_I256`, so it is not a constant expression.

<!-- Fix linking to `Uint`. See https://github.com/rust-lang/rust/issues/74481 -->

[`Uint`]: ../uint/struct.Uint.html
[`Int`]: ../uint/struct.Int.html
[`Bits`]: ../uint/struct.Bits.html
[rust-syntax]: https://doc.rust-lang.org/stable/reference/tokens.html#integer-literals

To use it simply import it in scope:

```rust
use ruint::uint;
```

Now constants can be created in decimal, hex, binary and even octal:

```rust
# use ruint::uint;
let avogadro = uint!(602_214_076_000_000_000_000_000_U256);
let cow_key = uint!(0xee79b5f6e221356af78cf4c36f4f7885a11b67dfcc81c34d80249947330c0f82_U256);
let bender = uint!(0b1010011010_U10);
let debt = uint!(-1_000_000_I256);
```

The [`uint!`] macro recurses through the parse tree, so the above can equivalently be written

```rust
# use ruint::uint;
uint! {
let avogadro = 602_214_076_000_000_000_000_000_U256;
let cow_key = 0xee79b5f6e221356af78cf4c36f4f7885a11b67dfcc81c34d80249947330c0f82_U256;
let bender = 0b1010011010_U10;
}
```

This latter form is particularly useful for lookup tables:

```rust
# use ruint::{Uint, uint};
const PRIMES: [Uint<128, 2>; 3] = uint!([
    170141183460469231731687303715884105757_U128,
    170141183460469231731687303715884105773_U128,
    170141183460469231731687303715884105793_U128,
]);
```

The macro will throw a compile time error if you try to create a constant that
does not fit the type:

```rust,compile_fail
# use ruint::uint;
# uint! {
let sparta = 300_U8;
# }
```

```text,ignore
error: Value too large for Uint<8>: 300
 --> src/example.rs:1:14
  |
1 | let sparta = 300_U8;
  |              ^^^^^^
```

`Int` literals must fit the type, except that `-2^(BITS-1)` is accepted right after a `-`.
Like Rust's own `-128i8.abs()`, a method call on such a literal does not compile, because the
call binds tighter than the `-`:

```rust
# use ruint::uint;
# uint! {
assert_eq!(-128_I8, ruint::aliases::I8::MIN);
# }
```

```rust,compile_fail
# use ruint::uint;
# uint! {
let wrong = -128_I8.unsigned_abs();
# }
```

## References

* Rust [integer literals syntax](https://doc.rust-lang.org/stable/reference/tokens.html#integer-literals).
