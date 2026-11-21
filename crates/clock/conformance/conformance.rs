//! The tests every implementation of wcontainers-clock runs, written against its traits alone.

use std::sync::{Mutex, PoisonError};
use std::thread;

use wcontainers_clock::Clock;

/// How many readings each check takes, enough to catch a clock that steps back only now and then.
const READINGS: usize = u16::MAX as usize;

/// Checks that the monotonic clock never goes backwards between readings in a row.
pub fn read_monotonic_clock_forward<C: Clock>(clocks: C) {
    let mut earlier = clocks.monotonic_nanoseconds();
    for _ in 0..READINGS {
        let later = clocks.monotonic_nanoseconds();
        assert!(
            later >= earlier,
            "the monotonic clock went from {earlier} back to {later}"
        );
        earlier = later;
    }
}

/// Checks that a monotonic reading taken after another on any thread is never smaller.
pub fn read_monotonic_clock_forward_across_threads<C: Clock + Sync>(clocks: C) {
    let threads = thread::available_parallelism()
        .expect("the host must say how many processors it has")
        .get();
    let latest = Mutex::new(clocks.monotonic_nanoseconds());
    thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                for _ in 0..READINGS / threads {
                    let mut latest = latest.lock().unwrap_or_else(PoisonError::into_inner);
                    let reading = clocks.monotonic_nanoseconds();
                    assert!(
                        reading >= *latest,
                        "the monotonic clock went from {} back to {reading} on another thread",
                        *latest
                    );
                    *latest = reading;
                }
            });
        }
    });
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn read_monotonic_clock_forward() {
            $crate::read_monotonic_clock_forward($value);
        }

        #[test]
        fn read_monotonic_clock_forward_across_threads() {
            $crate::read_monotonic_clock_forward_across_threads($value);
        }
    };
}
