//! Support for the [`rand`](https://crates.io/crates/rand) crate.

#![cfg(feature = "rand-010")]
#![cfg_attr(docsrs, doc(cfg(feature = "rand-010")))]

// FEATURE: Implement the Uniform distribution.

use rand_010 as rand;

use crate::Uint;
use rand::{
    Rng, RngExt,
    distr::{Distribution, StandardUniform},
};

impl<const BITS: usize, const LIMBS: usize> Distribution<Uint<BITS, LIMBS>> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Uint<BITS, LIMBS> {
        random_with(rng)
    }
}

// The inherent `random*` methods defer to `rand-09` or `rand` when enabled, so
// that enabling `rand-010` never changes their signatures.
#[cfg(not(any(feature = "rand", feature = "rand-09")))]
impl<const BITS: usize, const LIMBS: usize> Uint<BITS, LIMBS> {
    /// Creates a new [`Uint`] with the default cryptographic random number
    /// generator.
    ///
    /// This is currently [`rand::rng()`].
    #[inline]
    #[must_use]
    #[cfg(feature = "std")]
    pub fn random() -> Self {
        let mut uint = Self::ZERO;
        uint.randomize();
        uint
    }

    /// Creates a new [`Uint`] with the given random number generator.
    #[inline]
    #[doc(alias = "random_using")]
    #[must_use]
    pub fn random_with<R: rand::Rng + ?Sized>(rng: &mut R) -> Self {
        random_with(rng)
    }

    /// Fills this [`Uint`] with the default cryptographic random number
    /// generator.
    ///
    /// See [`random`](Self::random) for more details.
    #[inline]
    #[cfg(feature = "std")]
    pub fn randomize(&mut self) {
        self.randomize_with(&mut rand::rng());
    }

    /// Fills this [`Uint`] with the given random number generator.
    #[inline]
    #[doc(alias = "randomize_using")]
    pub fn randomize_with<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) {
        randomize_with(self, rng);
    }
}

#[inline]
fn random_with<const BITS: usize, const LIMBS: usize, R: rand::Rng + ?Sized>(
    rng: &mut R,
) -> Uint<BITS, LIMBS> {
    let mut uint = Uint::ZERO;
    randomize_with(&mut uint, rng);
    uint
}

#[inline]
fn randomize_with<const BITS: usize, const LIMBS: usize, R: rand::Rng + ?Sized>(
    uint: &mut Uint<BITS, LIMBS>,
    rng: &mut R,
) {
    rng.fill(&mut uint.limbs[..]);
    uint.apply_mask();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{const_for, nlimbs};

    #[test]
    fn test_rand() {
        let mut rng = rand::rng();
        const_for!(BITS in SIZES {
            const LIMBS: usize = nlimbs(BITS);
            for _ in 0..1000 {
                let _: Uint<BITS, LIMBS> = rng.random();
            }
        });
    }
}
