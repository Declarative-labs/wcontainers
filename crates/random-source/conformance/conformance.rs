//! The tests every implementation of wcontainers-random-source runs, written against its traits alone.

use core::mem::size_of;

use wcontainers_random_source::RandomSource;

/// The length of each random fill, the size of a `u128`.
const RANDOM_FILL_LENGTH: usize = size_of::<u128>();

/// Checks that two fills of random bytes differ.
pub fn fill_random_bytes_differently<R: RandomSource>(source: R) {
    let mut first = [u8::MIN; RANDOM_FILL_LENGTH];
    let mut second = [u8::MIN; RANDOM_FILL_LENGTH];
    source.fill_random_bytes(&mut first);
    source.fill_random_bytes(&mut second);
    assert_ne!(first, second);
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn fill_random_bytes_differently() {
            $crate::fill_random_bytes_differently($value);
        }
    };
}
