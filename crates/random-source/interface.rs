#![no_std]

use vstd::prelude::*;
use wcontainers_abi::HostError;
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

pub trait RandomSource {
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

pub trait StandaloneRandomSource: RandomSource {
    type Config;

    fn new(config: Self::Config) -> Self
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
