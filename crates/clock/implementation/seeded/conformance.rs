use wcontainers_clock::StandaloneClock;
use wcontainers_clock_seeded::{SeededClock, SeededClockConfig};

wcontainers_clock_conformance::tests!(SeededClock::new(SeededClockConfig { seed: u64::MIN }));
