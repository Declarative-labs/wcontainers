//! The tests every implementation of wcontainers-byte-sink runs, written against its traits alone.

use wcontainers_byte_sink::ByteSink;

/// The bytes written, as many as the size of a `u16`: fewer than each tested sink has room for.
const BYTES: [u8; size_of::<u16>()] = [u8::MAX; size_of::<u16>()];

/// Checks that a write takes no more bytes than it is given.
pub fn writes_at_most_the_bytes_given<T: ByteSink>(mut subject: T) {
    if let Ok(count) = subject.write(&BYTES) {
        assert!(count <= BYTES.len(), "a write took {count} of {} bytes", BYTES.len());
    }
}

/// Checks that a write of no bytes takes none.
pub fn writes_nothing_from_no_bytes<T: ByteSink>(mut subject: T) {
    if let Ok(count) = subject.write(&[]) {
        assert_eq!(count, usize::MIN);
    }
}

/// Runs each suite function above as its own test on a value made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn writes_at_most_the_bytes_given() {
            $crate::writes_at_most_the_bytes_given($value);
        }

        #[test]
        fn writes_nothing_from_no_bytes() {
            $crate::writes_nothing_from_no_bytes($value);
        }
    };
}
