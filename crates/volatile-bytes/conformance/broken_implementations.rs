//! The suite's own test: every suite function passes on bytes held correctly, and each broken implementation below, which
//! breaks one clause of the interface, fails the suite function named with it on one of the suite's own assertions.

use core::cell::RefCell;

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::VolatileBytes;

/// How many bytes each subject holds: two for every value of a byte, so that the suite's numbered bytes repeat and its
/// middle offset does not fit a byte.
const LENGTH: usize = 2 * (u8::MAX as usize + 1);

/// The bytes the store keeps after the end, which only the fault `WritesPastTheEnd` reaches.
const AFTER_THE_END: usize = size_of::<u64>();

/// One way an implementation breaks the interface.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    /// `len` counts one byte more than there is.
    LengthOneMore,
    /// `read_at` reads from the start, whatever the offset.
    ReadIgnoresTheOffset,
    /// `write_at` writes to the start, whatever the offset.
    WriteIgnoresTheOffset,
    /// `read_at` keeps only the low byte of the offset.
    ReadCutsTheOffsetToAByte,
    /// `read_at` from an offset past the start copies only the first byte, though it counts them all.
    ReadAtAnOffsetCopiesOnlyTheFirstByte,
    /// `read_at` past the end reads the bytes the store keeps after it: its bound refuses only the length itself.
    ReadPastTheEndReadsTheStore,
    /// `read` fills a buffer shorter than the bytes from the end instead of the start.
    ReadFillsAShortBufferFromTheEnd,
    /// `read` counts one byte more for a buffer longer than the bytes.
    ReadCountsALongerBufferOneMore,
    /// `read_at` reads from the byte after the offset when one is left.
    ReadsOneByteLate,
    /// `read_at` counts one byte more than it copies when the buffer is shorter than what is left.
    ReadCountsOneMore,
    /// `write_at` also writes the byte after when the buffer is shorter than what is left, and counts it.
    WritesOneByteMore,
    /// `read_at` copies and counts one byte fewer.
    ReadsOneByteFewer,
    /// `write_at` copies and counts one byte fewer.
    WritesOneByteFewer,
    /// `read_at` writes the buffer beyond the bytes it copies.
    ReadTouchesTheBufferBeyondTheCount,
    /// `write_at` takes its bound from the store, which holds bytes after the end.
    WritesPastTheEnd,
    /// `read_at` refuses the last byte.
    ReadRefusesTheLastByte,
    /// `read_at` at the length reads the last byte.
    ReadAtTheLengthReadsTheLastByte,
    /// `read_at` keeps only the low 32 bits of the offset, as a 32-bit address would.
    ReadTruncatesTheOffset,
    /// `read_at` reads from the start when the offset plus the buffer's length wraps.
    ReadWrapsTheOffset,
    /// `read_at` counts one byte for an empty buffer.
    ReadCountsAnEmptyBuffer,
    /// `read` reads from the byte after the start.
    ReadNotFromTheStart,
    /// `write` writes to the byte after the start.
    WriteNotFromTheStart,
    /// `read_to` gives the sink one byte fewer.
    ReadToGivesOneByteFewer,
    /// `write_from` lets the source fill one byte fewer.
    WriteFromTakesOneByteFewer,
    /// `write_from` counts the length instead of what the source gave.
    WriteFromCountsTheLength,
}

/// Bytes held in a vector, exactly as the interface says unless a fault is given.
struct Held {
    store: RefCell<Vec<u8>>,
    fault: Option<Fault>,
}

impl Held {
    fn new(fault: Option<Fault>) -> Held {
        Held { store: RefCell::new(vec![u8::MIN; LENGTH + AFTER_THE_END]), fault }
    }

    fn breaks(&self, fault: Fault) -> bool {
        self.fault == Some(fault)
    }

    /// Returns where the offset lies and how many bytes from it a buffer of `buffer` bytes takes, within `bound` bytes.
    fn place(offset: u64, buffer: usize, bound: usize) -> (usize, usize) {
        match usize::try_from(offset) {
            Ok(at) if at <= bound => (at, buffer.min(bound - at)),
            _ => (usize::MIN, usize::MIN),
        }
    }
}

impl VolatileBytes for Held {
    fn len(&self) -> u64 {
        let length = if self.breaks(Fault::LengthOneMore) { LENGTH + 1 } else { LENGTH };
        length as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let store = self.store.borrow();
        let offset = if self.breaks(Fault::ReadTruncatesTheOffset) { u64::from(offset as u32) } else { offset };
        let offset = if self.breaks(Fault::ReadCutsTheOffsetToAByte) { u64::from(offset as u8) } else { offset };
        let wraps = offset.checked_add(into.len() as u64).is_none();
        let (mut at, mut count) = Held::place(offset, into.len(), LENGTH);
        if self.breaks(Fault::ReadPastTheEndReadsTheStore) && offset > LENGTH as u64 {
            (at, count) = Held::place(offset, into.len(), LENGTH + AFTER_THE_END);
        }
        if self.breaks(Fault::ReadIgnoresTheOffset) || (self.breaks(Fault::ReadWrapsTheOffset) && wraps) {
            (at, count) = Held::place(u64::MIN, into.len(), LENGTH);
        }
        if self.breaks(Fault::ReadAtTheLengthReadsTheLastByte) && at == LENGTH && !into.is_empty() {
            (at, count) = (LENGTH - 1, 1);
        }
        if self.breaks(Fault::ReadRefusesTheLastByte) && at + count == LENGTH {
            count = count.saturating_sub(1);
        }
        if self.breaks(Fault::ReadsOneByteFewer) {
            count = count.saturating_sub(1);
        }
        let from = if self.breaks(Fault::ReadsOneByteLate) && at + count < LENGTH { at + 1 } else { at };
        let copied = if self.breaks(Fault::ReadAtAnOffsetCopiesOnlyTheFirstByte) && at > usize::MIN {
            count.min(1)
        } else {
            count
        };
        into[..copied].copy_from_slice(&store[from..from + copied]);
        if self.breaks(Fault::ReadTouchesTheBufferBeyondTheCount) {
            into[count..].fill(u8::MIN);
        }
        if self.breaks(Fault::ReadCountsOneMore) && count == into.len() && at + count < LENGTH {
            count += 1;
        }
        if self.breaks(Fault::ReadCountsAnEmptyBuffer) && into.is_empty() {
            count = 1;
        }
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let mut store = self.store.borrow_mut();
        let bound = if self.breaks(Fault::WritesPastTheEnd) { LENGTH + AFTER_THE_END } else { LENGTH };
        let (mut at, mut count) = Held::place(offset, bytes.len(), bound);
        if self.breaks(Fault::WriteIgnoresTheOffset) {
            at = usize::MIN;
        }
        if self.breaks(Fault::WritesOneByteFewer) {
            count = count.saturating_sub(1);
        }
        store[at..at + count].copy_from_slice(&bytes[..count]);
        if self.breaks(Fault::WritesOneByteMore) && count > usize::MIN && count == bytes.len() && at + count < LENGTH {
            store[at + count] = bytes[count - 1];
            count += 1;
        }
        count
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let mut offset = if self.breaks(Fault::ReadNotFromTheStart) { 1 } else { u64::MIN };
        if self.breaks(Fault::ReadFillsAShortBufferFromTheEnd) {
            offset = LENGTH.saturating_sub(into.len()) as u64;
        }
        let count = self.read_at(offset, into);
        if self.breaks(Fault::ReadCountsALongerBufferOneMore) && into.len() > LENGTH { count + 1 } else { count }
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let offset = if self.breaks(Fault::WriteNotFromTheStart) { 1 } else { u64::MIN };
        self.write_at(offset, bytes)
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let length = if self.breaks(Fault::ReadToGivesOneByteFewer) { LENGTH - 1 } else { LENGTH };
        let mut bytes = vec![u8::MIN; length];
        let count = self.read(&mut bytes);
        sink.write(&bytes[..count])
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let length = if self.breaks(Fault::WriteFromTakesOneByteFewer) { LENGTH - 1 } else { LENGTH };
        let mut bytes = vec![u8::MIN; length];
        let count = source.read(&mut bytes)?;
        let written = self.write(&bytes[..count]);
        Ok(if self.breaks(Fault::WriteFromCountsTheLength) { LENGTH } else { written })
    }
}

mod held_correctly {
    wcontainers_volatile_bytes_conformance::tests!(super::Held::new(None));
}

/// Makes, for each fault, a test that the suite function named with it fails on bytes with that fault. The panic expected
/// is the start of the message of the suite's `assert!` and `assert_eq!`, so that a panic of the broken implementation
/// itself is not taken for the suite catching it.
macro_rules! caught {
    ($($test:ident: $fault:ident by $function:ident;)*) => {
        $(
            #[test]
            #[should_panic(expected = "assertion")]
            fn $test() {
                wcontainers_volatile_bytes_conformance::$function(Held::new(Some(Fault::$fault)));
            }
        )*
    };
}

caught! {
    a_length_one_more: LengthOneMore by reads_back_what_was_written;
    a_write_one_byte_fewer: WritesOneByteFewer by reads_back_what_was_written;
    a_write_one_byte_more: WritesOneByteMore by writes_only_the_start;
    a_sink_given_one_byte_fewer: ReadToGivesOneByteFewer by reads_all_the_bytes_to_a_sink;
    a_source_let_fill_one_byte_fewer: WriteFromTakesOneByteFewer by writes_all_the_bytes_from_a_source;
    a_source_counted_as_the_length: WriteFromCountsTheLength by writes_only_what_a_source_holds;
    a_read_ignoring_the_offset: ReadIgnoresTheOffset by reads_and_writes_at_an_offset_inside;
    a_read_one_byte_late: ReadsOneByteLate by reads_and_writes_at_an_offset_inside;
    a_read_of_an_offset_cut_to_a_byte: ReadCutsTheOffsetToAByte by reads_and_writes_at_an_offset_inside;
    a_read_at_an_offset_copying_only_the_first_byte: ReadAtAnOffsetCopiesOnlyTheFirstByte by reads_and_writes_at_an_offset_inside;
    a_read_ignoring_the_offset_of_a_longer_buffer: ReadIgnoresTheOffset by reads_and_writes_what_is_left_of_a_longer_buffer;
    a_read_past_the_end_of_the_store: ReadPastTheEndReadsTheStore by reads_and_writes_nothing_past_the_end;
    a_read_of_a_short_buffer_from_the_end: ReadFillsAShortBufferFromTheEnd by reads_and_writes_as_from_the_start;
    a_read_counting_a_longer_buffer_one_more: ReadCountsALongerBufferOneMore by reads_and_writes_as_from_the_start;
    a_read_counting_one_more: ReadCountsOneMore by reads_and_writes_at_an_offset_inside;
    a_read_one_byte_fewer: ReadsOneByteFewer by reads_and_writes_the_last_byte;
    a_read_refusing_the_last_byte: ReadRefusesTheLastByte by reads_and_writes_the_last_byte;
    a_read_at_the_length_of_the_last_byte: ReadAtTheLengthReadsTheLastByte by reads_and_writes_nothing_at_the_length;
    a_write_past_the_end: WritesPastTheEnd by reads_and_writes_nothing_at_the_length;
    a_read_of_a_truncated_offset: ReadTruncatesTheOffset by reads_and_writes_nothing_past_the_end;
    a_read_of_a_wrapping_offset: ReadWrapsTheOffset by reads_and_writes_nothing_at_the_largest_offset;
    a_read_touching_the_buffer_beyond_the_count: ReadTouchesTheBufferBeyondTheCount by reads_and_writes_what_is_left_of_a_longer_buffer;
    a_read_counting_an_empty_buffer: ReadCountsAnEmptyBuffer by reads_and_writes_nothing_with_an_empty_buffer;
    a_write_ignoring_the_offset: WriteIgnoresTheOffset by writes_at_an_offset_leave_every_other_byte;
    a_read_not_from_the_start: ReadNotFromTheStart by reads_and_writes_as_from_the_start;
    a_write_not_from_the_start: WriteNotFromTheStart by reads_and_writes_as_from_the_start;
}
