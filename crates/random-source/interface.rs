#![no_std]

use wcontainers_abi::HostError;
use wcontainers_volatile_bytes::VolatileBytes;

pub trait RandomSource {
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError>;
}

pub trait StandaloneRandomSource: RandomSource {
    type Config;

    fn new(config: Self::Config) -> Self;
}
