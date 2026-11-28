#![no_std]

/// A clock whose monotonic reading never goes backwards, and which follows real time on a best effort unless its implementation has reason not to.
#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait Clock {
    fn monotonic_nanoseconds(&self) -> u64;
    fn realtime_nanoseconds(&self) -> u64;
}

#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait StandaloneClock: Clock {
    type Config;

    fn new(config: Self::Config) -> Self;
}
