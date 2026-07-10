// TODO: https://baincapitalcrypto.com/optimizing-montgomery-multiplication-in-webassembly/

use super::{DW, borrowing_sub, carrying_add, cmp};
use crate::utils::select_unpredictable;
use core::{cmp::Ordering, iter::zip};

/// ⚠️ Computes a * b * 2^(-BITS) mod modulus
#[doc = crate::algorithms::unstable_warning!()]
/// Requires that `inv` is the inverse of `-modulus[0]` modulo `2^64`.
/// Requires that `a` and `b` are less than `modulus`.
#[inline]
#[must_use]
pub fn mul_redc<const N: usize>(a: [u64; N], b: [u64; N], modulus: [u64; N], inv: u64) -> [u64; N] {
    debug_assert_eq!(inv.wrapping_mul(modulus[0]), u64::MAX);
    debug_assert_eq!(cmp(&a, &modulus), Ordering::Less);
    debug_assert_eq!(cmp(&b, &modulus), Ordering::Less);

    // Coarsely Integrated Operand Scanning (CIOS)
    // See <https://www.microsoft.com/en-us/research/wp-content/uploads/1998/06/97Acar.pdf>
    // See <https://hackmd.io/@gnark/modular_multiplication#fn1>
    // See <https://tches.iacr.org/index.php/TCHES/article/view/10972>
    let mut result = [0; N];
    let mut carry = false;
    for b in b {
        let mut m = 0;
        let mut carry_1 = 0;
        let mut carry_2 = 0;
        for i in 0..N {
            // Add limb product
            let (value, next_carry) = carrying_mul_add(a[i], b, result[i], carry_1);
            carry_1 = next_carry;

            if i == 0 {
                // Compute reduction factor
                m = value.wrapping_mul(inv);
            }

            // Add m * modulus to acc to clear next_result[0]
            let (value, next_carry) = carrying_mul_add(modulus[i], m, value, carry_2);
            carry_2 = next_carry;

            // Shift result
            if i > 0 {
                result[i - 1] = value;
            } else {
                debug_assert_eq!(value, 0);
            }
        }

        // Add carries
        let (value, next_carry) = carrying_add(carry_1, carry_2, carry);
        result[N - 1] = value;
        if modulus[N - 1] >= 0x7fff_ffff_ffff_ffff {
            carry = next_carry;
        } else {
            debug_assert!(!next_carry);
        }
    }

    // Compute reduced product.
    reduce1_carry(result, modulus, carry)
}

/// ⚠️ Computes a^2 * 2^(-BITS) mod modulus
#[doc = crate::algorithms::unstable_warning!()]
/// Requires that `inv` is the inverse of `-modulus[0]` modulo `2^64`.
/// Requires that `a` is less than `modulus`.
#[inline]
#[must_use]
pub fn square_redc<const N: usize>(a: [u64; N], modulus: [u64; N], inv: u64) -> [u64; N] {
    debug_assert_eq!(inv.wrapping_mul(modulus[0]), u64::MAX);
    debug_assert_eq!(cmp(&a, &modulus), Ordering::Less);

    // The specialized path below beats the plain multiply only while it
    // compiles to straight-line code; past 8 limbs the loops no longer
    // unroll and the multiply's fused inner loop wins (measured on Apple
    // M-series and AMD Zen 2).
    if N > 8 {
        return mul_redc(a, a, modulus, inv);
    }

    // The 2N-limb square is accumulated in `(lo, hi)`, `lo` holding limbs
    // 0..N and `hi` limbs N..2N. All loops below run a constant `N`
    // iterations with the triangular structure expressed as guards, and all
    // carries fit u64 or bool, so the code fully unrolls and the carries
    // lower to flag registers. (A single loop with data-dependent trip count
    // or a two-word carry defeats both.)
    let mut lo = [0; N];
    let mut hi = [0; N];

    // Cross products, undoubled: sum of a[i] * a[j] for i < j.
    for i in 0..N {
        let mut carry = 0;
        for j in 0..N {
            if j > i {
                let (value, next_carry) = carrying_mul_add(a[i], a[j], get(&lo, &hi, i + j), carry);
                set(&mut lo, &mut hi, i + j, value);
                carry = next_carry;
            }
        }
        if i + 1 < N {
            // Row i ends at limb i + N - 1; its carry lands in fresh limb
            // i + N. The last row is empty and carries nothing.
            set(&mut lo, &mut hi, i + N, carry);
        }
    }

    // Double the cross products (their sum is less than 2^(128N - 1), so
    // the shift cannot overflow) and add the diagonal squares, in one pass.
    // Limb pairs (2i, 2i + 1) are contiguous, so a single shift-out bit and
    // a single carry chain cover the whole sweep.
    let mut msb = 0;
    let mut carry = false;
    for (i, &limb) in a.iter().enumerate() {
        let (square_lo, square_hi) = carrying_mul_add(limb, limb, 0, 0);
        let value = get(&lo, &hi, 2 * i);
        let (doubled, next_msb) = ((value << 1) | msb, value >> 63);
        let (value, next_carry) = carrying_add(doubled, square_lo, carry);
        set(&mut lo, &mut hi, 2 * i, value);
        msb = next_msb;
        let value = get(&lo, &hi, 2 * i + 1);
        let (doubled, next_msb) = ((value << 1) | msb, value >> 63);
        let (value, next_carry) = carrying_add(doubled, square_hi, next_carry);
        set(&mut lo, &mut hi, 2 * i + 1, value);
        msb = next_msb;
        carry = next_carry;
    }
    debug_assert_eq!(msb, 0);
    debug_assert!(!carry);

    // Montgomery reduction, with `lo` as a sliding window: each round clears
    // the window's bottom limb by adding m * modulus, shifts the window down
    // one limb, and pulls in the next limb of `hi`. In fixed-index terms,
    // round i's final add lands at limb i + N, whose carry-out belongs to
    // limb i + N + 1 — exactly the next round's final add, so `carry_top`
    // hands it forward.
    let mut carry_top = false;
    for &hi_limb in &hi {
        let m = lo[0].wrapping_mul(inv);
        let (value, mut carry) = carrying_mul_add(m, modulus[0], lo[0], 0);
        debug_assert_eq!(value, 0);
        for j in 1..N {
            let (value, next_carry) = carrying_mul_add(modulus[j], m, lo[j], carry);
            lo[j - 1] = value;
            carry = next_carry;
        }
        let (value, next_carry) = carrying_add(hi_limb, carry, carry_top);
        lo[N - 1] = value;
        carry_top = next_carry;
    }

    // The reduced square is a^2 / 2^(64N) + (sum of m_i * modulus) / 2^(64N)
    // < modulus^2 / 2^(64N) + modulus < 2 * modulus, so one conditional
    // subtraction (with `carry_top` as the 2^(64N) bit) completes it.
    reduce1_carry(lo, modulus, carry_top)
}

/// Reads limb `i` of the 2N-limb value split across `lo` and `hi`.
#[inline(always)]
fn get<const N: usize>(lo: &[u64; N], hi: &[u64; N], i: usize) -> u64 {
    if i < N { lo[i] } else { hi[i - N] }
}

/// Writes limb `i` of the 2N-limb value split across `lo` and `hi`.
#[inline(always)]
fn set<const N: usize>(lo: &mut [u64; N], hi: &mut [u64; N], i: usize, value: u64) {
    if i < N {
        lo[i] = value;
    } else {
        hi[i - N] = value;
    }
}

#[inline]
#[must_use]
#[allow(clippy::needless_bitwise_bool)]
fn reduce1_carry<const N: usize>(value: [u64; N], modulus: [u64; N], carry: bool) -> [u64; N] {
    let (reduced, borrow) = sub(value, modulus);
    select_unpredictable(carry | !borrow, reduced, value)
}

#[inline]
#[must_use]
fn sub<const N: usize>(lhs: [u64; N], rhs: [u64; N]) -> ([u64; N], bool) {
    let mut result = [0; N];
    let mut borrow = false;
    for (result, (lhs, rhs)) in zip(&mut result, zip(lhs, rhs)) {
        let (value, next_borrow) = borrowing_sub(lhs, rhs, borrow);
        *result = value;
        borrow = next_borrow;
    }
    (result, borrow)
}

/// Compute `lhs * rhs + add + carry`.
/// The output can not overflow for any input values.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
fn carrying_mul_add(lhs: u64, rhs: u64, add: u64, carry: u64) -> (u64, u64) {
    DW::split(DW::muladd2(lhs, rhs, add, carry))
}

#[cfg(test)]
mod test {
    use super::{
        super::{addmul, div},
        *,
    };
    use crate::{Uint, aliases::U64, const_for, nlimbs};
    use core::ops::Neg;
    use proptest::{prop_assert_eq, proptest};

    fn modmul<const N: usize>(a: [u64; N], b: [u64; N], modulus: [u64; N]) -> [u64; N] {
        // Compute a * b
        let mut product = vec![0; 2 * N];
        addmul(&mut product, &a, &b);

        // Compute product mod modulus
        let mut reduced = modulus;
        div(&mut product, &mut reduced);
        reduced
    }

    fn mul_base<const N: usize>(a: [u64; N], modulus: [u64; N]) -> [u64; N] {
        // Compute a * 2^(N * 64)
        let mut product = vec![0; 2 * N];
        product[N..].copy_from_slice(&a);

        // Compute product mod modulus
        let mut reduced = modulus;
        div(&mut product, &mut reduced);
        reduced
    }

    #[test]
    fn test_mul_redc() {
        const_for!(BITS in NON_ZERO if BITS >= 16 {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(mut a: U, mut b: U, mut m: U)| {
                m |= U::from(1_u64); // Make sure m is odd.
                a %= m; // Make sure a is less than m.
                b %= m; // Make sure b is less than m.
                let a = *a.as_limbs();
                let b = *b.as_limbs();
                let m = *m.as_limbs();
                let inv = U64::from(m[0]).inv_ring().unwrap().neg().as_limbs()[0];

                let result = mul_base(mul_redc(a, b, m, inv), m);
                let expected = modmul(a, b, m);

                prop_assert_eq!(result, expected);
            });
        });
    }

    #[test]
    fn test_square_redc() {
        const_for!(BITS in NON_ZERO if BITS >= 16 {
            const LIMBS: usize = nlimbs(BITS);
            type U = Uint<BITS, LIMBS>;
            proptest!(|(mut a: U, mut m: U)| {
                m |= U::from(1_u64); // Make sure m is odd.
                a %= m; // Make sure a is less than m.
                let a = *a.as_limbs();
                let m = *m.as_limbs();
                let inv = U64::from(m[0]).inv_ring().unwrap().neg().as_limbs()[0];

                let result = mul_base(square_redc(a, m, inv), m);
                let expected = modmul(a, a, m);

                prop_assert_eq!(result, expected);
            });
        });
    }
}
