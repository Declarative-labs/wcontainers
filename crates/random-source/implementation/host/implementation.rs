//! The operating system's random bytes, fit for cryptography, through the getrandom crate.

use wcontainers_abi::HostError;
use wcontainers_random_source::{RandomSource, StandaloneRandomSource};

#[derive(Debug)]
pub struct HostRandomSource;

impl StandaloneRandomSource for HostRandomSource {
    type Config = ();

    fn new((): Self::Config) -> Self {
        Self
    }
}

impl RandomSource for HostRandomSource {
    fn fill_random_bytes(&self, into: &mut [u8]) -> Result<(), HostError> {
        getrandom::fill(into).map_err(|error| {
            error
                .raw_os_error()
                .map_or(HostError::NotSupported, HostError::Errno)
        })
    }
}
