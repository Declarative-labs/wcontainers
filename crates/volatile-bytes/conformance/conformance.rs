//! The tests every implementation of wcontainers-volatile-bytes runs, written against its traits alone.

use wcontainers_volatile_bytes::VolatileBytes;

/// The byte the tests write.
const WRITTEN: u8 = u8::MAX;

/// Returns the number of bytes the subject holds, which the tests need to be more than none.
fn count_bytes<T: VolatileBytes>(subject: &T) -> usize {
    let length = usize::try_from(subject.len()).expect("the bytes are no longer than the host can address");
    assert!(!subject.is_empty() && length > usize::MIN, "the suite needs bytes that hold at least one byte");
    length
}

/// Checks that the bytes written to the start are the bytes read back, and that a buffer longer than the bytes is cut to them.
pub fn reads_back_what_was_written<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let longer = vec![WRITTEN; length + size_of::<u64>()];
    assert_eq!(subject.write(&longer), length);
    let mut read = vec![u8::MIN; length + size_of::<u64>()];
    assert_eq!(subject.read(&mut read), length);
    assert_eq!(read[..length], longer[..length]);
    assert!(read[length..].iter().all(|&byte| byte == u8::MIN));
}

/// Checks that a write shorter than the bytes changes only the start.
pub fn writes_only_the_start<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    assert_eq!(subject.write(&vec![u8::MIN; length]), length);
    assert_eq!(subject.write(&[WRITTEN]), size_of::<u8>());
    let mut read = vec![WRITTEN; length];
    assert_eq!(subject.read(&mut read), length);
    assert_eq!(read[0], WRITTEN);
    assert!(read[1..].iter().all(|&byte| byte == u8::MIN));
}

/// Checks that a sink receives all the bytes in one write.
pub fn reads_all_the_bytes_to_a_sink<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    assert_eq!(subject.write(&vec![WRITTEN; length]), length);
    let mut sink = Vec::new();
    assert_eq!(subject.read_to(&mut sink).ok(), Some(length));
    assert_eq!(sink, vec![WRITTEN; length]);
}

/// Checks that a source longer than the bytes fills all of them in one read and keeps the rest.
pub fn writes_all_the_bytes_from_a_source<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let held = vec![WRITTEN; length + size_of::<u64>()];
    let mut source = held.as_slice();
    assert_eq!(subject.write_from(&mut source).ok(), Some(length));
    assert_eq!(source.len(), size_of::<u64>());
    let mut read = vec![u8::MIN; length];
    assert_eq!(subject.read(&mut read), length);
    assert_eq!(read, held[..length]);
}

/// Checks that a source shorter than the bytes changes only the start.
pub fn writes_only_what_a_source_holds<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    assert_eq!(subject.write(&vec![u8::MIN; length]), length);
    let mut source: &[u8] = &[WRITTEN];
    assert_eq!(subject.write_from(&mut source).ok(), Some(size_of::<u8>()));
    let mut read = vec![WRITTEN; length];
    assert_eq!(subject.read(&mut read), length);
    assert_eq!(read[0], WRITTEN);
    assert!(read[1..].iter().all(|&byte| byte == u8::MIN));
}

/// Runs each suite function above as its own test on a value made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn reads_back_what_was_written() {
            $crate::reads_back_what_was_written($value);
        }

        #[test]
        fn writes_only_the_start() {
            $crate::writes_only_the_start($value);
        }

        #[test]
        fn reads_all_the_bytes_to_a_sink() {
            $crate::reads_all_the_bytes_to_a_sink($value);
        }

        #[test]
        fn writes_all_the_bytes_from_a_source() {
            $crate::writes_all_the_bytes_from_a_source($value);
        }

        #[test]
        fn writes_only_what_a_source_holds() {
            $crate::writes_only_what_a_source_holds($value);
        }
    };
}
