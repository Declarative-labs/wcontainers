//! Random bytes from a seed: the same bytes in the same order for the same seed, not fit for cryptography.

use std::convert::Infallible;
use std::sync::{Mutex, PoisonError};

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use wcontainers_abi::HostError;
use wcontainers_byte_source::ByteSource;
use wcontainers_random_source::{RandomSource, StandaloneRandomSource};
use wcontainers_volatile_bytes::VolatileBytes;

#[derive(Debug)]
pub struct SeededRandomSource {
    generator: Mutex<ChaCha8Rng>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeededRandomSourceConfig {
    pub seed: u64,
}

/// The generator as a source of bytes, which fills every buffer it is given.
struct GeneratedBytes<'generator> {
    generator: &'generator mut ChaCha8Rng,
}

impl ByteSource for GeneratedBytes<'_> {
    type Error = Infallible;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Infallible> {
        self.generator.fill_bytes(into);
        Ok(into.len())
    }
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
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError> {
        let mut generator = self
            .generator
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Ok(written) = into.write_from(&mut GeneratedBytes {
            generator: &mut generator,
        });
        if u64::try_from(written).ok() == Some(into.len()) {
            Ok(())
        } else {
            Err(HostError::NotSupported)
        }
    }
}
