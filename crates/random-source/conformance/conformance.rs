//! The tests every implementation of wcontainers-random-source runs, written against its traits alone.

use core::mem::size_of;
use std::thread;

use wcontainers_random_source::RandomSource;

/// The length of each random fill, the size of a `u128`.
const RANDOM_FILL_LENGTH: usize = size_of::<u128>();

/// A fill long enough to hold every byte value, missing one by chance far less than once in 2^128.
const LONG_FILL_LENGTH: usize = u16::MAX as usize + 1;

/// The lengths filled: none, one, a fill, and a long fill.
const FILL_LENGTHS: &[usize] = &[usize::MIN, 1, RANDOM_FILL_LENGTH, LONG_FILL_LENGTH];

/// Checks that two fills of random bytes differ.
pub fn fill_random_bytes_differently<R: RandomSource>(source: R) {
    let mut first = [u8::MIN; RANDOM_FILL_LENGTH];
    let mut second = [u8::MIN; RANDOM_FILL_LENGTH];
    source
        .fill_random_bytes(&mut first)
        .expect("the random source must fill what it is given");
    source
        .fill_random_bytes(&mut second)
        .expect("the random source must fill what it is given");
    assert_ne!(first, second);
}

/// Checks that every byte of what is given is written, in one of a fill's worth of fills.
pub fn fill_every_byte_given<R: RandomSource>(source: R) {
    for &length in FILL_LENGTHS {
        let mut written = vec![false; length];
        let mut buffer = vec![u8::MIN; length];
        for _ in 0..RANDOM_FILL_LENGTH {
            buffer.fill(u8::MIN);
            source
                .fill_random_bytes(&mut buffer)
                .expect("the random source must fill what it is given");
            for (written, &byte) in written.iter_mut().zip(&buffer) {
                *written |= byte != u8::MIN;
            }
        }
        let unwritten = written.iter().position(|&written| !written);
        assert_eq!(
            unwritten, None,
            "a fill of {length} bytes left a byte unwritten"
        );
    }
}

/// Checks that one long fill holds every byte value.
pub fn fill_random_bytes_with_every_byte_value<R: RandomSource>(source: R) {
    let mut buffer = vec![u8::MIN; LONG_FILL_LENGTH];
    source
        .fill_random_bytes(&mut buffer)
        .expect("the random source must fill what it is given");
    let mut seen = [false; u8::MAX as usize + 1];
    for &byte in &buffer {
        seen[usize::from(byte)] = true;
    }
    let missing = seen.iter().position(|&seen| !seen);
    assert_eq!(missing, None, "a long fill lacked a byte value");
}

/// Checks that fills made at once on every processor all differ.
pub fn fill_random_bytes_differently_across_threads<R: RandomSource + Sync>(source: R) {
    let threads = thread::available_parallelism()
        .expect("the host must say how many processors it has")
        .get();
    let mut fills = thread::scope(|scope| {
        let fillers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut fill = [u8::MIN; RANDOM_FILL_LENGTH];
                    source
                        .fill_random_bytes(&mut fill)
                        .expect("the random source must fill what it is given");
                    fill
                })
            })
            .collect();
        fillers
            .into_iter()
            .map(|filler| filler.join().expect("a filling thread panicked"))
            .collect::<Vec<_>>()
    });
    fills.sort_unstable();
    fills.dedup();
    assert_eq!(fills.len(), threads, "two threads' fills were equal");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn fill_random_bytes_differently() {
            $crate::fill_random_bytes_differently($value);
        }

        #[test]
        fn fill_every_byte_given() {
            $crate::fill_every_byte_given($value);
        }

        #[test]
        fn fill_random_bytes_with_every_byte_value() {
            $crate::fill_random_bytes_with_every_byte_value($value);
        }

        #[test]
        fn fill_random_bytes_differently_across_threads() {
            $crate::fill_random_bytes_differently_across_threads($value);
        }
    };
}
