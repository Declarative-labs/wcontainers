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

/// Returns the bytes the offset tests fill the subject with: each differs from the bytes beside it and from `WRITTEN`, so
/// a copy one byte off reads or writes a value that shows it.
fn numbered(count: usize) -> Vec<u8> {
    (0..count).map(|index| (index % usize::from(u8::MAX)) as u8).collect()
}

/// Fills the subject with `numbered` bytes and returns them.
fn fill_numbered<T: VolatileBytes>(subject: &T, length: usize) -> Vec<u8> {
    let bytes = numbered(length);
    assert_eq!(subject.write(&bytes), length);
    bytes
}

/// Returns all the subject's bytes.
fn read_all<T: VolatileBytes>(subject: &T, length: usize) -> Vec<u8> {
    let mut read = vec![WRITTEN; length];
    assert_eq!(subject.read(&mut read), length);
    read
}

/// Returns an index into the subject as an offset.
fn offset_of(index: usize) -> u64 {
    u64::try_from(index).expect("an index fits an offset")
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

/// Checks that a read from an offset inside copies the bytes from there, a buffer shorter than what is left taking only
/// its own length, and that a write there changes exactly the buffer's bytes.
pub fn reads_and_writes_at_an_offset_inside<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let mut expected = fill_numbered(&subject, length);
    let offset = length / 2;
    let count = (length - offset).div_ceil(2);
    let mut read = vec![WRITTEN; count + 1];
    assert_eq!(subject.read_at(offset_of(offset), &mut read[..count]), count);
    assert_eq!(read[..count], expected[offset..offset + count]);
    assert_eq!(read[count], WRITTEN);
    assert_eq!(subject.write_at(offset_of(offset), &vec![WRITTEN; count]), count);
    expected[offset..offset + count].fill(WRITTEN);
    assert_eq!(read_all(&subject, length), expected);
}

/// Checks that an offset at the last byte reads and writes that one byte, whatever the buffer's length.
pub fn reads_and_writes_the_last_byte<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let mut expected = fill_numbered(&subject, length);
    let last = offset_of(length - 1);
    let mut read = [WRITTEN; size_of::<u64>()];
    assert_eq!(subject.read_at(last, &mut read), size_of::<u8>());
    assert_eq!(read[0], expected[length - 1]);
    assert!(read[1..].iter().all(|&byte| byte == WRITTEN));
    assert_eq!(subject.write_at(last, &[WRITTEN; size_of::<u64>()]), size_of::<u8>());
    expected[length - 1] = WRITTEN;
    assert_eq!(read_all(&subject, length), expected);
}

/// Checks that an offset equal to the length reads and writes nothing, and is no error.
pub fn reads_and_writes_nothing_at_the_length<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let expected = fill_numbered(&subject, length);
    let mut read = [WRITTEN; size_of::<u64>()];
    assert_eq!(subject.read_at(offset_of(length), &mut read), usize::MIN);
    assert_eq!(read, [WRITTEN; size_of::<u64>()]);
    assert_eq!(subject.write_at(offset_of(length), &[WRITTEN; size_of::<u64>()]), usize::MIN);
    assert_eq!(read_all(&subject, length), expected);
}

/// Checks that offsets past the end read and write nothing: one past it; 2^63, whose low 32 bits are zero, so that a
/// 32-bit address cut from it would be the start; and one whose sum with the buffer's length wraps.
pub fn reads_and_writes_nothing_past_the_end<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let expected = fill_numbered(&subject, length);
    for offset in [offset_of(length) + 1, 1 << (u64::BITS - 1), u64::MAX - (size_of::<u64>() as u64 - 1)] {
        let mut read = [WRITTEN; 2 * size_of::<u64>()];
        assert_eq!(subject.read_at(offset, &mut read), usize::MIN, "{offset}");
        assert_eq!(read, [WRITTEN; 2 * size_of::<u64>()], "{offset}");
        assert_eq!(subject.write_at(offset, &[WRITTEN; 2 * size_of::<u64>()]), usize::MIN, "{offset}");
        assert_eq!(read_all(&subject, length), expected, "{offset}");
    }
}

/// Checks that the largest offset reads and writes nothing.
pub fn reads_and_writes_nothing_at_the_largest_offset<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let expected = fill_numbered(&subject, length);
    let mut read = vec![WRITTEN; length];
    assert_eq!(subject.read_at(u64::MAX, &mut read), usize::MIN);
    assert_eq!(read, vec![WRITTEN; length]);
    assert_eq!(subject.write_at(u64::MAX, &vec![WRITTEN; length]), usize::MIN);
    assert_eq!(read_all(&subject, length), expected);
}

/// Checks that a buffer longer than what is left from the offset is cut to it, and that the rest of the buffer is left
/// as it was.
pub fn reads_and_writes_what_is_left_of_a_longer_buffer<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let mut expected = fill_numbered(&subject, length);
    let offset = length / 2;
    let left = length - offset;
    let mut read = vec![WRITTEN; left + size_of::<u64>()];
    assert_eq!(subject.read_at(offset_of(offset), &mut read), left);
    assert_eq!(read[..left], expected[offset..]);
    assert!(read[left..].iter().all(|&byte| byte == WRITTEN));
    assert_eq!(subject.write_at(offset_of(offset), &vec![WRITTEN; left + size_of::<u64>()]), left);
    expected[offset..].fill(WRITTEN);
    assert_eq!(read_all(&subject, length), expected);
}

/// Checks that an empty buffer reads and writes nothing at any offset inside or at the end.
pub fn reads_and_writes_nothing_with_an_empty_buffer<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let expected = fill_numbered(&subject, length);
    for offset in [0, length / 2, length - 1, length].map(offset_of) {
        assert_eq!(subject.read_at(offset, &mut []), usize::MIN, "{offset}");
        assert_eq!(subject.write_at(offset, &[]), usize::MIN, "{offset}");
    }
    assert_eq!(read_all(&subject, length), expected);
}

/// Checks that a write at an offset leaves every byte before and after the bytes it writes as it was.
pub fn writes_at_an_offset_leave_every_other_byte<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let mut expected = fill_numbered(&subject, length);
    for offset in [0, length / 2, length - 1] {
        assert_eq!(subject.write_at(offset_of(offset), &[WRITTEN]), size_of::<u8>(), "{offset}");
        expected[offset] = WRITTEN;
        assert_eq!(read_all(&subject, length), expected, "{offset}");
    }
}

/// Checks that `read` and `write` are `read_at` and `write_at` from the start, for a buffer shorter than, as long as and
/// longer than the bytes.
pub fn reads_and_writes_as_from_the_start<T: VolatileBytes>(subject: T) {
    let length = count_bytes(&subject);
    let expected = fill_numbered(&subject, length);
    for buffer in [length - 1, length, length + size_of::<u64>()] {
        let (mut read, mut read_at) = (vec![WRITTEN; buffer], vec![WRITTEN; buffer]);
        let count = buffer.min(length);
        assert_eq!(subject.read(&mut read), count, "{buffer}");
        assert_eq!(subject.read_at(u64::MIN, &mut read_at), count, "{buffer}");
        assert_eq!(read, read_at, "{buffer}");
        assert_eq!(read[..count], expected[..count], "{buffer}");
    }
    let mut written = numbered(length - 1);
    written.reverse();
    let mut changed = expected.clone();
    changed[..length - 1].copy_from_slice(&written);
    assert_eq!(subject.write(&written), length - 1);
    assert_eq!(read_all(&subject, length), changed);
    assert_eq!(subject.write(&expected), length);
    assert_eq!(subject.write_at(u64::MIN, &written), length - 1);
    assert_eq!(read_all(&subject, length), changed);
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

        #[test]
        fn reads_and_writes_at_an_offset_inside() {
            $crate::reads_and_writes_at_an_offset_inside($value);
        }

        #[test]
        fn reads_and_writes_the_last_byte() {
            $crate::reads_and_writes_the_last_byte($value);
        }

        #[test]
        fn reads_and_writes_nothing_at_the_length() {
            $crate::reads_and_writes_nothing_at_the_length($value);
        }

        #[test]
        fn reads_and_writes_nothing_past_the_end() {
            $crate::reads_and_writes_nothing_past_the_end($value);
        }

        #[test]
        fn reads_and_writes_nothing_at_the_largest_offset() {
            $crate::reads_and_writes_nothing_at_the_largest_offset($value);
        }

        #[test]
        fn reads_and_writes_what_is_left_of_a_longer_buffer() {
            $crate::reads_and_writes_what_is_left_of_a_longer_buffer($value);
        }

        #[test]
        fn reads_and_writes_nothing_with_an_empty_buffer() {
            $crate::reads_and_writes_nothing_with_an_empty_buffer($value);
        }

        #[test]
        fn writes_at_an_offset_leave_every_other_byte() {
            $crate::writes_at_an_offset_leave_every_other_byte($value);
        }

        #[test]
        fn reads_and_writes_as_from_the_start() {
            $crate::reads_and_writes_as_from_the_start($value);
        }
    };
}
