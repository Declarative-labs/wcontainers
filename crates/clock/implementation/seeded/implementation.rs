//! Simulated time from a seed: the same readings in the same order for the same seed.

use core::time::Duration;
use std::sync::{Mutex, PoisonError};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use wcontainers_clock::Clock;

#[derive(Debug)]
pub struct SeededClock {
    time: Mutex<SimulatedTime>,
}

#[derive(Debug)]
struct SimulatedTime {
    generator: ChaCha8Rng,
    monotonic: u64,
    /// The wall clock's distance ahead of the monotonic clock.
    realtime_offset: u64,
}

impl SeededClock {
    pub fn new(seed: u64) -> Self {
        let mut generator = ChaCha8Rng::seed_from_u64(seed);
        // The monotonic clock starts within a u32 of nanoseconds, as a machine's does soon after it starts.
        let monotonic = u64::from(generator.random::<u32>());
        // The wall clock starts within a u32 of seconds after 1970.
        let realtime_offset =
            u64::try_from(Duration::from_secs(generator.random::<u32>().into()).as_nanos())
                .expect("a u32 of seconds fits a u64 of nanoseconds");
        Self {
            time: Mutex::new(SimulatedTime {
                generator,
                monotonic,
                realtime_offset,
            }),
        }
    }

    /// Advances simulated time by up to a u16 of nanoseconds and gives the new monotonic reading and the offset.
    fn advance(&self) -> (u64, u64) {
        let mut time = self.time.lock().unwrap_or_else(PoisonError::into_inner);
        let step = u64::from(time.generator.random::<u16>());
        time.monotonic = time
            .monotonic
            .checked_add(step)
            .expect("simulated time must fit a u64 of nanoseconds");
        (time.monotonic, time.realtime_offset)
    }
}

impl Clock for SeededClock {
    fn monotonic_nanoseconds(&self) -> u64 {
        self.advance().0
    }

    fn realtime_nanoseconds(&self) -> u64 {
        let (monotonic, realtime_offset) = self.advance();
        monotonic
            .checked_add(realtime_offset)
            .expect("simulated time must fit a u64 of nanoseconds")
    }
}
