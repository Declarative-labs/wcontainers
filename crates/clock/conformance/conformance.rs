//! The tests every implementation of wcontainers-clock runs, written against its traits alone.

use wcontainers_clock::Clock;

/// Checks that the monotonic clock never goes backwards between two reads.
pub fn read_monotonic_clock_forward<C: Clock>(clocks: C) {
    let earlier = clocks.monotonic_nanoseconds();
    assert!(clocks.monotonic_nanoseconds() >= earlier);
}

/// Calls each method of `Clock` as a placeholder that fails until the real checks are written.
pub fn call_clock_methods<C: Clock>(clocks: C) {
    clocks.monotonic_nanoseconds();
    clocks.realtime_nanoseconds();
    todo!("this is a placeholder: the real checks of Clock are not written yet");
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
        fn call_clock_methods() {
            $crate::call_clock_methods($value);
        }
    };
}
