//! Random bytes from a seed: the same bytes in the same order for the same seed, not fit for cryptography.

use std::sync::{Mutex, PoisonError};

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use wcontainers_random_source::RandomSource;

#[derive(Debug)]
pub struct SeededRandomSource {
    generator: Mutex<ChaCha8Rng>,
}

impl SeededRandomSource {
    pub fn new(seed: u64) -> Self {
        Self {
            generator: Mutex::new(ChaCha8Rng::seed_from_u64(seed)),
        }
    }
}

impl RandomSource for SeededRandomSource {
    fn fill_random_bytes(&self, into: &mut [u8]) {
        self.generator
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .fill_bytes(into);
    }
}
