//! The operating system's clocks, read through [clock_gettime](https://man7.org/linux/man-pages/man2/clock_gettime.2.html).

use core::time::Duration;

use rustix::time::{ClockId, clock_gettime};
use wcontainers_clock::{Clock, StandaloneClock};

#[derive(Debug)]
pub struct HostClock;

impl StandaloneClock for HostClock {
    type Config = ();

    fn new((): Self::Config) -> Self {
        Self
    }
}

impl Clock for HostClock {
    fn monotonic_nanoseconds(&self) -> u64 {
        nanoseconds(ClockId::Monotonic)
    }

    fn realtime_nanoseconds(&self) -> u64 {
        nanoseconds(ClockId::Realtime)
    }
}

fn nanoseconds(clock: ClockId) -> u64 {
    let since_zero = Duration::try_from(clock_gettime(clock))
        .expect("the host's clock must not be before its zero");
    u64::try_from(since_zero.as_nanos()).expect("the host's clock must fit a u64 of nanoseconds")
}
