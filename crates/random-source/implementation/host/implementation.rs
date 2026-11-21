//! The operating system's random bytes, fit for cryptography, through the getrandom crate.

use wcontainers_random_source::RandomSource;

#[derive(Debug)]
pub struct HostRandomSource;

impl RandomSource for HostRandomSource {
    fn fill_random_bytes(&self, into: &mut [u8]) {
        if let Err(error) = getrandom::fill(into) {
            panic!("the host's random source failed: {error}");
        }
    }
}
