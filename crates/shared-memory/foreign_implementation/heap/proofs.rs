//! Kani's checks of the range arithmetic of every method, over a heap mapping of each count of words from 1 to `WORDS`
//! (one harness per count: a vector of a symbolic size makes the solver run out of memory), and every offset, length and
//! content. Kani cannot call the C library, so the copies call a stand-in built on `copy_nonoverlapping`, whose ranges
//! Kani checks are valid and apart; each check of bytes picks one byte at any index, so that it covers them all without a
//! loop. An empty mapping is left to the tests.
//! Run with `cargo kani -p wcontainers-shared-memory -Z stubbing`.

use alloc::vec::Vec;

use wcontainers_volatile_bytes::VolatileBytes;

use crate::{HeapMapping, OutsideError, SharedMemory, Word, WordError};

/// The most words a mapping holds in these checks.
const WORDS: usize = 4;

/// The bytes of one word.
const WORD: usize = size_of::<u64>();

/// The most bytes a mapping holds in these checks.
const BYTES: usize = WORDS * WORD;

/// Returns a mapping of the count of words, each byte holding any value, and those values.
fn any_memory(count: usize) -> (HeapMapping, [u8; BYTES]) {
    let memory = HeapMapping::new(count);
    let held: [u8; BYTES] = kani::any();
    for index in 0..count {
        let mut value = [0; WORD];
        value.copy_from_slice(&held[index * WORD..(index + 1) * WORD]);
        assert_eq!(
            memory.store((index * WORD) as u64, u64::from_ne_bytes(value)),
            Ok(())
        );
    }
    (memory, held)
}

/// Returns the number of bytes mapped.
fn bytes(memory: &HeapMapping) -> usize {
    memory.len() as usize
}

/// Returns the byte at the offset, which lies inside the mapping, read through the 8-byte word that holds it.
fn byte(memory: &HeapMapping, at: usize) -> u8 {
    memory
        .load::<u64>((at - at % WORD) as u64)
        .expect("the word lies inside the mapping")
        .to_ne_bytes()[at % WORD]
}

/// Checks that the byte at any offset of the mapping is the one `expected` gives for it.
fn check_any_byte(memory: &HeapMapping, expected: impl Fn(usize) -> u8) {
    let at: usize = kani::any();
    if at < bytes(memory) {
        assert_eq!(byte(memory, at), expected(at));
    }
}

/// Returns what a word method must return for a word of `size` bytes at the offset: inside, aligned, or neither.
fn expected_word(memory: &HeapMapping, at: u64, size: usize) -> Result<(), WordError> {
    if u128::from(at) + size as u128 > bytes(memory) as u128 {
        Err(WordError::Outside(OutsideError))
    } else if at % size as u64 != 0 {
        Err(WordError::Misaligned)
    } else {
        Ok(())
    }
}

fn counts_the_bytes_mapped(count: usize) {
    assert_eq!(HeapMapping::new(count).len(), (count * WORD) as u64);
}

/// The stand-in for the foreign call to `memcpy`, with the same contract.
///
/// # Safety
/// `from` is readable and `to` writable for `length` bytes, and the two ranges do not overlap.
#[expect(
    unsafe_code,
    reason = "Kani's stand-in for the foreign call, which Kani checks"
)]
unsafe fn copy(to: *mut u8, from: *const u8, length: usize) {
    // SAFETY: the caller's promise, which is what Kani checks here.
    unsafe { core::ptr::copy_nonoverlapping(from, to, length) }
}

/// Checks load, store and the method only words of this size have, at any offset: each succeeds exactly when the word
/// lies inside and is aligned, returns what the word held, and changes only the word's own bytes; `change` returns the
/// value it left in the word.
fn words_of_one_size<W: Word + kani::Arbitrary, const SIZE: usize>(
    count: usize,
    to_bytes: fn(W) -> [u8; SIZE],
    change: fn(&HeapMapping, u64, W, W, Result<(), WordError>) -> W,
) {
    let (memory, before) = any_memory(count);
    let (at, value, other): (u64, W, W) = (kani::any(), kani::any(), kani::any());
    let expected = expected_word(&memory, at, SIZE);
    let inside = expected.is_ok();
    kani::cover!(
        inside && at as usize + SIZE == bytes(&memory),
        "the last word of the mapping"
    );
    assert_eq!(memory.load::<W>(at).map(|_| ()), expected);
    assert_eq!(memory.store(at, value), expected);
    let value = change(&memory, at, value, other, expected);
    if inside {
        assert_eq!(memory.load::<W>(at), Ok(value));
    }
    let first = at as usize;
    check_any_byte(&memory, |byte| {
        if inside && (first..first + SIZE).contains(&byte) {
            to_bytes(value)[byte - first]
        } else {
            before[byte]
        }
    });
}

/// Swaps `other` into the word that holds `value`, checks it returns `value`, and returns `other`.
fn swap(memory: &HeapMapping, at: u64, value: u64, other: u64, expected: Result<(), WordError>) -> u64 {
    assert_eq!(memory.swap(at, other), expected.map(|()| value));
    other
}

/// Exchanges with a value compared that the word does not hold and then with one it holds, checks both return `value`,
/// and returns what the second left.
fn compare_exchange(
    memory: &HeapMapping,
    at: u64,
    value: u32,
    other: u32,
    expected: Result<(), WordError>,
) -> u32 {
    assert_eq!(
        memory.compare_exchange(at, !value, other),
        expected.map(|()| value)
    );
    if expected.is_ok() {
        assert_eq!(memory.load::<u32>(at), Ok(value));
    }
    assert_eq!(
        memory.compare_exchange(at, value, other),
        expected.map(|()| value)
    );
    other
}

fn reads_exactly_the_bytes_asked(count: usize) {
    let (memory, before) = any_memory(count);
    let at: u64 = kani::any();
    let length = kani::any_where(|length: &usize| *length <= BYTES + 1);
    let original: [u8; BYTES + 1] = kani::any();
    let mut buffer = original;
    let inside = u128::from(at) + length as u128 <= bytes(&memory) as u128;
    kani::cover!(
        inside && at % WORD as u64 == 3 && at + length as u64 == bytes(&memory) as u64 - 1,
        "a read from a word's fourth byte to the last word's seventh"
    );
    assert_eq!(
        memory.read(at, &mut buffer[..length]),
        if inside { Ok(()) } else { Err(OutsideError) }
    );
    let index: usize = kani::any();
    if index <= BYTES {
        let expected = if inside && index < length {
            before[at as usize + index]
        } else {
            original[index]
        };
        assert_eq!(buffer[index], expected);
    }
    check_any_byte(&memory, |at| before[at]);
}

fn writes_exactly_the_bytes_given(count: usize) {
    let (memory, before) = any_memory(count);
    let at: u64 = kani::any();
    let length = kani::any_where(|length: &usize| *length <= BYTES + 1);
    let written: [u8; BYTES + 1] = kani::any();
    let inside = u128::from(at) + length as u128 <= bytes(&memory) as u128;
    kani::cover!(
        inside && at % WORD as u64 == 3 && at + length as u64 == bytes(&memory) as u64 - 1,
        "a write from a word's fourth byte to the last word's seventh"
    );
    assert_eq!(
        memory.write(at, &written[..length]),
        if inside { Ok(()) } else { Err(OutsideError) }
    );
    let first = at as usize;
    check_any_byte(&memory, |byte| {
        if inside && (first..first + length).contains(&byte) {
            written[byte - first]
        } else {
            before[byte]
        }
    });
}

fn parts_lie_inside_and_reach_only_their_bytes(count: usize) {
    let (memory, before) = any_memory(count);
    let (start, end): (u64, u64) = (kani::any(), kani::any());
    let part = memory.borrow(start..end);
    assert_eq!(part.is_ok(), start <= end && end <= bytes(&memory) as u64);
    let Ok(part) = part else { return };
    kani::cover!(
        start % WORD as u64 == 3 && end == bytes(&memory) as u64 - 1,
        "a part from a word's fourth byte to the last word's seventh"
    );
    let start = start as usize;
    assert_eq!(part.len(), end - start as u64);
    let length = kani::any_where(|length: &usize| *length <= BYTES);
    let count = length.min(part.len() as usize);
    let mut buffer = [0; BYTES];
    assert_eq!(VolatileBytes::read(&part, &mut buffer[..length]), count);
    let index: usize = kani::any();
    if index < count {
        assert_eq!(buffer[index], before[start + index]);
    }
    let written: [u8; BYTES] = kani::any();
    assert_eq!(VolatileBytes::write(&part, &written[..length]), count);
    check_any_byte(&memory, |at| {
        if (start..start + count).contains(&at) {
            written[at - start]
        } else {
            before[at]
        }
    });
}

fn parts_read_to_a_sink_and_write_from_a_source(count: usize) {
    let (memory, before) = any_memory(count);
    let (start, end) = (
        kani::any_where(|start: &usize| *start <= BYTES),
        kani::any_where(|end: &usize| *end <= BYTES),
    );
    kani::assume(start <= end && end <= bytes(&memory));
    let part = memory
        .borrow(start as u64..end as u64)
        .expect("the offsets lie inside the mapping");
    let mut sink = Vec::new();
    assert_eq!(part.read_to(&mut sink), Ok(end - start));
    let index: usize = kani::any();
    if index < end - start {
        assert_eq!(sink[index], before[start + index]);
    }
    let written: [u8; BYTES] = kani::any();
    let length = kani::any_where(|length: &usize| *length <= BYTES);
    let mut source = &written[..length];
    let count = length.min(end - start);
    let Ok(written_count) = part.write_from(&mut source);
    assert_eq!(written_count, count);
    check_any_byte(&memory, |at| {
        if (start..start + count).contains(&at) {
            written[at - start]
        } else {
            before[at]
        }
    });
}

/// The futex call, replaced by one that reports a timeout for a wait and wakes as many threads as asked.
#[cfg(target_os = "linux")]
fn futex(
    _: *mut u32,
    operation: crate::foreign_implementation::region::FutexOperation,
    value: u32,
    _: Option<&libc::timespec>,
) -> Result<usize, rustix::io::Errno> {
    if operation == crate::foreign_implementation::region::FutexOperation::Wait {
        Err(rustix::io::Errno::TIMEDOUT)
    } else {
        Ok(value as usize)
    }
}

#[cfg(target_os = "linux")]
fn waits_and_wakes_only_on_words_inside(count: usize) {
    use crate::WaitError;

    let (memory, _) = any_memory(count);
    let (at, expected, limit): (u64, u32, u32) = (kani::any(), kani::any(), kani::any());
    let word = expected_word(&memory, at, size_of::<u32>()).map_err(WaitError::Word);
    assert_eq!(memory.wait(at, expected, None), word);
    assert_eq!(memory.wake(at, limit), word.map(|()| limit));
}

/// Makes one harness of each check above for each count of words.
macro_rules! harnesses {
    ($($count:literal => $module:ident),*) => {$(
        mod $module {
            #[kani::proof]
            fn counts_the_bytes_mapped() {
                super::counts_the_bytes_mapped($count);
            }

            #[kani::proof]
            #[kani::unwind(6)]
            fn loads_stores_and_swaps_only_8_byte_words_inside() {
                super::words_of_one_size::<u64, { size_of::<u64>() }>(
                    $count,
                    u64::to_ne_bytes,
                    super::swap,
                );
            }

            #[kani::proof]
            #[kani::unwind(6)]
            fn loads_stores_and_exchanges_only_4_byte_words_inside() {
                super::words_of_one_size::<u32, { size_of::<u32>() }>(
                    $count,
                    u32::to_ne_bytes,
                    super::compare_exchange,
                );
            }

            #[kani::proof]
            #[kani::unwind(35)]
            #[kani::stub(crate::foreign_implementation::region::copy, super::copy)]
            fn reads_exactly_the_bytes_asked() {
                super::reads_exactly_the_bytes_asked($count);
            }

            #[kani::proof]
            #[kani::unwind(35)]
            #[kani::stub(crate::foreign_implementation::region::copy, super::copy)]
            fn writes_exactly_the_bytes_given() {
                super::writes_exactly_the_bytes_given($count);
            }

            #[kani::proof]
            #[kani::unwind(35)]
            #[kani::stub(crate::foreign_implementation::region::copy, super::copy)]
            fn parts_lie_inside_and_reach_only_their_bytes() {
                super::parts_lie_inside_and_reach_only_their_bytes($count);
            }

            #[kani::proof]
            #[kani::unwind(35)]
            #[kani::stub(crate::foreign_implementation::region::copy, super::copy)]
            fn parts_read_to_a_sink_and_write_from_a_source() {
                super::parts_read_to_a_sink_and_write_from_a_source($count);
            }

            #[cfg(target_os = "linux")]
            #[kani::proof]
            #[kani::unwind(6)]
            #[kani::stub(crate::foreign_implementation::region::futex, super::futex)]
            fn waits_and_wakes_only_on_words_inside() {
                super::waits_and_wakes_only_on_words_inside($count);
            }
        }
    )*};
}

harnesses!(1 => one_word, 2 => two_words, 3 => three_words, 4 => four_words);
