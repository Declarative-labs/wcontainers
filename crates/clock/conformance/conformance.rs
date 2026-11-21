//! The tests every implementation of wcontainers-clock runs, written against its traits alone.

use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wcontainers_clock::Clock;

/// How many readings each check takes, enough to catch a clock that steps back only now and then.
const READINGS: usize = u16::MAX as usize;

/// A sleep far longer than a clock's [resolution](https://man7.org/linux/man-pages/man2/clock_getres.2.html), and short enough to keep the suite quick.
const SLEEP: Duration = Duration::from_millis(1);

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

/// Checks that monotonic readings around a sleep differ by its length in nanoseconds, as a reference clock sees it.
pub fn count_monotonic_nanoseconds_across_a_sleep<C: Clock>(clocks: C) {
    let reference_start = Instant::now();
    let start = clocks.monotonic_nanoseconds();
    thread::sleep(SLEEP);
    let end = clocks.monotonic_nanoseconds();
    let reference = reference_start.elapsed().as_nanos();
    let elapsed = u128::from(
        end.checked_sub(start)
            .expect("the monotonic clock went backwards across a sleep"),
    );
    assert!(
        elapsed >= SLEEP.as_nanos(),
        "the monotonic clock counted {elapsed} across a sleep of {} nanoseconds",
        SLEEP.as_nanos()
    );
    assert!(
        elapsed <= 2 * reference,
        "the monotonic clock counted {elapsed} where the reference counted {reference} nanoseconds"
    );
}

/// Checks that a realtime reading is the wall clock's nanoseconds since 1970, between two reference readings.
pub fn read_realtime_clock_as_the_wall_clock<C: Clock>(clocks: C) {
    let since_epoch = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the host's wall clock must be after the epoch")
            .as_nanos()
    };
    let before = since_epoch();
    let reading = u128::from(clocks.realtime_nanoseconds());
    let after = since_epoch();
    assert!(
        (before..=after).contains(&reading),
        "the realtime clock read {reading} between wall clock readings {before} and {after} nanoseconds since the epoch"
    );
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

        #[test]
        fn count_monotonic_nanoseconds_across_a_sleep() {
            $crate::count_monotonic_nanoseconds_across_a_sleep($value);
        }

        #[test]
        fn read_realtime_clock_as_the_wall_clock() {
            $crate::read_realtime_clock_as_the_wall_clock($value);
        }
    };
}
