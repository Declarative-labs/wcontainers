#![no_std]

/// A clock whose monotonic reading never goes backwards, and which follows real time on a best effort unless its implementation has reason not to.
use vstd::prelude::*;

verus! {

pub trait Clock {
    fn monotonic_nanoseconds(&self) -> u64
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn realtime_nanoseconds(&self) -> u64
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

pub trait StandaloneClock: Clock {
    type Config;

    fn new(config: Self::Config) -> Self
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
