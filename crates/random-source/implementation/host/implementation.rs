//! The operating system's random bytes, fit for cryptography, through the getrandom crate.

use wcontainers_abi::HostError;
use wcontainers_byte_source::ByteSource;
use wcontainers_random_source::{RandomSource, StandaloneRandomSource};
use wcontainers_volatile_bytes::VolatileBytes;

#[derive(Debug)]
pub struct HostRandomSource;

/// The operating system's random bytes as a source, which fills every buffer it is given or fails.
struct OperatingSystemBytes;

impl ByteSource for OperatingSystemBytes {
    type Error = HostError;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, HostError> {
        getrandom::fill(into).map_err(|error| {
            error
                .raw_os_error()
                .map_or(HostError::NotSupported, HostError::Errno)
        })?;
        Ok(into.len())
    }
}

impl StandaloneRandomSource for HostRandomSource {
    type Config = ();

    fn new((): Self::Config) -> Self {
        Self
    }
}

impl RandomSource for HostRandomSource {
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError> {
        let written = into.write_from(&mut OperatingSystemBytes)?;
        if u64::try_from(written).ok() == Some(into.len()) {
            Ok(())
        } else {
            Err(HostError::NotSupported)
        }
    }
}
