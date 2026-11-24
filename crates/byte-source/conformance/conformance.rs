//! The tests every implementation of wcontainers-byte-source runs, written against its traits alone.

use wcontainers_byte_source::ByteSource;

/// The length of the buffer read into, the size of a `u16`: shorter than what each tested source holds.
const BUFFER_LENGTH: usize = size_of::<u16>();

/// Checks that a read returns no more bytes than its buffer holds.
pub fn reads_at_most_the_buffer<T: ByteSource>(mut subject: T) {
    let mut buffer = [u8::MIN; BUFFER_LENGTH];
    if let Ok(count) = subject.read(&mut buffer) {
        assert!(count <= buffer.len(), "a read returned {count} bytes for a buffer of {}", buffer.len());
    }
}

/// Checks that a read into an empty buffer returns zero.
pub fn reads_nothing_into_an_empty_buffer<T: ByteSource>(mut subject: T) {
    if let Ok(count) = subject.read(&mut []) {
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
        fn reads_at_most_the_buffer() {
            $crate::reads_at_most_the_buffer($value);
        }

        #[test]
        fn reads_nothing_into_an_empty_buffer() {
            $crate::reads_nothing_into_an_empty_buffer($value);
        }
    };
}
