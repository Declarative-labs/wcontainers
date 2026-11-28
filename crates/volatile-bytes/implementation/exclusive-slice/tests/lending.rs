//! Reads and writes through the bytes while `read_to` or `write_from` has lent the slice, which count and move nothing.

use core::convert::Infallible;

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_exclusive_slice::ExclusiveSliceBytes;

/// The number of bytes each test holds.
const LENGTH: usize = u8::MAX as usize;
/// The byte the bytes hold before a test.
const HELD: u8 = u8::MAX;
/// The byte a test writes or fills its buffers with.
const OTHER: u8 = u8::MIN;

/// What a call made while the slice was lent returned, and the buffer it read into.
#[derive(Default)]
struct Seen {
    counts: Vec<usize>,
    buffers: Vec<Vec<u8>>,
    lent: Vec<Option<usize>>,
}

/// Returns the offsets the tests read and write at: the start, the middle, the last byte, the length and past it.
fn offsets() -> [u64; 5] {
    let length = u64::try_from(LENGTH).expect("the length fits an offset");
    [u64::MIN, length / 2, length - 1, length, u64::MAX]
}

/// Returns how many bytes a copy at the offset moves in `LENGTH` bytes, as the contract's `copied` says.
fn copied(offset: u64, buffer: usize) -> usize {
    match usize::try_from(offset) {
        Ok(at) if at < LENGTH => buffer.min(LENGTH - at),
        _ => usize::MIN,
    }
}

/// Reads at every offset, writes at every offset and lends again, recording each result.
fn reenter(bytes: &ExclusiveSliceBytes<'_>, seen: &mut Seen) {
    for offset in offsets() {
        let mut buffer = vec![OTHER; LENGTH + 1];
        seen.counts.push(bytes.read_at(offset, &mut buffer));
        seen.buffers.push(buffer);
        seen.counts.push(bytes.write_at(offset, &[OTHER; LENGTH + 1]));
    }
    seen.lent.push(bytes.read_to(&mut Vec::new()).ok());
    seen.lent.push(bytes.write_from(&mut [OTHER; LENGTH].as_slice()).ok());
}

/// A sink that reads and writes through the bytes it is being written from.
struct ReenteringSink<'b, 'a> {
    bytes: &'b ExclusiveSliceBytes<'a>,
    seen: Seen,
}

impl ByteSink for ReenteringSink<'_, '_> {
    type Error = Infallible;

    fn write(&mut self, written: &[u8]) -> Result<usize, Infallible> {
        reenter(self.bytes, &mut self.seen);
        Ok(written.len())
    }
}

/// A source that reads and writes through the bytes it is being read into, then fills them with `OTHER`.
struct ReenteringSource<'b, 'a> {
    bytes: &'b ExclusiveSliceBytes<'a>,
    seen: Seen,
}

impl ByteSource for ReenteringSource<'_, '_> {
    type Error = Infallible;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Infallible> {
        reenter(self.bytes, &mut self.seen);
        into.fill(OTHER);
        Ok(into.len())
    }
}

/// Checks what a call made while the slice was lent returned: the count `copied` says, an untouched buffer, `Ok(0)`.
fn check(seen: &Seen) {
    let expected: Vec<usize> = offsets().into_iter().flat_map(|offset| [copied(offset, LENGTH + 1); 2]).collect();
    assert_eq!(seen.counts, expected);
    assert!(seen.buffers.iter().all(|buffer| buffer.iter().all(|&byte| byte == OTHER)));
    assert_eq!(seen.lent, vec![Some(usize::MIN); 2]);
}

#[test]
fn counts_and_moves_nothing_while_a_sink_has_the_slice() {
    let mut held = [HELD; LENGTH];
    let bytes = ExclusiveSliceBytes::new(&mut held[..]);
    let mut sink = ReenteringSink { bytes: &bytes, seen: Seen::default() };
    assert_eq!(bytes.read_to(&mut sink).ok(), Some(LENGTH));
    check(&sink.seen);
    let mut read = [OTHER; LENGTH];
    assert_eq!(bytes.read(&mut read), LENGTH);
    assert_eq!(read, [HELD; LENGTH]);
}

#[test]
fn counts_and_moves_nothing_while_a_source_has_the_slice() {
    let mut held = [HELD; LENGTH];
    let bytes = ExclusiveSliceBytes::new(&mut held[..]);
    let mut source = ReenteringSource { bytes: &bytes, seen: Seen::default() };
    assert_eq!(bytes.write_from(&mut source).ok(), Some(LENGTH));
    check(&source.seen);
    let mut read = [HELD; LENGTH];
    assert_eq!(bytes.read(&mut read), LENGTH);
    assert_eq!(read, [OTHER; LENGTH]);
}

#[test]
fn reads_and_writes_again_once_the_slice_is_back() {
    let mut held = [HELD; LENGTH];
    let bytes = ExclusiveSliceBytes::new(&mut held[..]);
    let mut sink = ReenteringSink { bytes: &bytes, seen: Seen::default() };
    assert_eq!(bytes.read_to(&mut sink).ok(), Some(LENGTH));
    assert_eq!(bytes.write_at(u64::MIN, &[OTHER]), 1);
    let mut read = [HELD; 1];
    assert_eq!(bytes.read_at(u64::MIN, &mut read), 1);
    assert_eq!(read, [OTHER]);
    assert_eq!(bytes.read_to(&mut Vec::new()).ok(), Some(LENGTH));
}
