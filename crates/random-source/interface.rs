#![no_std]

use wcontainers_abi::HostError;
use wcontainers_volatile_bytes::VolatileBytes;

#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait RandomSource {
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError>;
}

#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait StandaloneRandomSource: RandomSource {
    type Config;

    fn new(config: Self::Config) -> Self;
}
