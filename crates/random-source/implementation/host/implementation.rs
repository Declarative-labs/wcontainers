//! The operating system's random bytes, fit for cryptography, through the getrandom crate.

use wcontainers_abi::HostError;
use wcontainers_byte_source::ByteSource;
use wcontainers_random_source::{RandomSource, StandaloneRandomSource};
use wcontainers_volatile_bytes::VolatileBytes;

#[derive(Debug)]
pub struct HostRandomSource;

impl StandaloneRandomSource for HostRandomSource {
    type Config = ();

    fn new((): Self::Config) -> Self {
        Self
    }
}

impl RandomSource for HostRandomSource {
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError> {
        into.write_from(&mut OperatingSystemBytes)
            .map(drop)
            .map_err(|error| {
                error
                    .raw_os_error()
                    .map_or(HostError::NotSupported, HostError::Errno)
            })
    }
}

/// The operating system's random bytes, read in one call into the whole buffer.
struct OperatingSystemBytes;

impl ByteSource for OperatingSystemBytes {
    type Error = getrandom::Error;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, getrandom::Error> {
        getrandom::fill(into)?;
        Ok(into.len())
    }
}
