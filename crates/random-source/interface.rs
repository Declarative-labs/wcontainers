#![no_std]

use wcontainers_abi::HostError;

pub trait RandomSource {
    fn fill_random_bytes(&self, into: &mut [u8]) -> Result<(), HostError>;
}

pub trait StandaloneRandomSource: RandomSource {
    type Config;

    fn new(config: Self::Config) -> Self;
}
