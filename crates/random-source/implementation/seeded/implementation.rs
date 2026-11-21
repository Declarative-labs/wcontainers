//! Random bytes from a seed: the same bytes in the same order for the same seed, not fit for cryptography.

use std::sync::{Mutex, PoisonError};

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use wcontainers_random_source::{RandomSource, StandaloneRandomSource};

#[derive(Debug)]
pub struct SeededRandomSource {
    generator: Mutex<ChaCha8Rng>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeededRandomSourceConfig {
    pub seed: u64,
}

impl StandaloneRandomSource for SeededRandomSource {
    type Config = SeededRandomSourceConfig;

    fn new(config: Self::Config) -> Self {
        Self {
            generator: Mutex::new(ChaCha8Rng::seed_from_u64(config.seed)),
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
