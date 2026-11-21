#![no_std]

pub trait Clock {
    fn monotonic_nanoseconds(&self) -> u64;
    fn realtime_nanoseconds(&self) -> u64;
}
