#![no_std]

use vstd::prelude::*;

verus! {

/// A clock whose monotonic reading never goes backwards, and which follows real time on a best effort unless its implementation has reason not to.
pub trait Clock {
    fn monotonic_nanoseconds(&self) -> u64
        ensures
            contract_not_written(),
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
    where
        Self: Sized,
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// TRIAL: the contract of a method whose contract is not written yet, which nothing can prove.
pub uninterp spec fn contract_not_written() -> bool;

/// TRIAL: a clock that always reads zero, to see what Verus says of each method.
pub struct ZeroClock;

impl Clock for ZeroClock {
    fn monotonic_nanoseconds(&self) -> u64 {
        0
    }

    fn realtime_nanoseconds(&self) -> u64 {
        0
    }
}

} // verus!
