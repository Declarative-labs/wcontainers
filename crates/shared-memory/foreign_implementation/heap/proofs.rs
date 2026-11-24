//! Kani's checks of every method of `Words` and `Part`, over a heap mapping of each count of words from 1 to `WORDS`
//! (one harness per count: a box of a symbolic size makes the solver run out of 130 GB), and every offset, length and
//! content. Each check of bytes picks one byte at any index, so that it covers them all without a loop. An empty
//! mapping is left to the tests: Kani cannot follow the dangling address of an empty box.
//! Run with `cargo kani -p wcontainers-shared-memory -Z stubbing`.

use alloc::vec::Vec;

use wcontainers_volatile_bytes::VolatileBytes;

use crate::{HeapMapping, OutsideError, Words};

/// The most words a mapping holds in these checks.
const WORDS: usize = 4;

/// The bytes of one word.
const WORD: usize = size_of::<u64>();

/// The most bytes a mapping holds in these checks.
const BYTES: usize = WORDS * WORD;

/// Returns words over a mapping of the count, each holding any value, and those values as bytes.
fn any_words(count: usize) -> (Words, [u8; BYTES]) {
    let words = Words::new(HeapMapping::new(count));
    let held: [u8; BYTES] = kani::any();
    for index in 0..count {
        let mut value = [0; WORD];
        value.copy_from_slice(&held[index * WORD..(index + 1) * WORD]);
        assert_eq!(words.store(index as u64, u64::from_ne_bytes(value)), Ok(()));
    }
    (words, held)
}

/// Returns the number of bytes mapped.
fn bytes(words: &Words) -> usize {
    words.count() as usize * WORD
}

/// Returns the byte at the offset, which lies inside the mapping.
fn byte(words: &Words, at: usize) -> u8 {
    words
        .load((at / WORD) as u64)
        .expect("the word lies inside the mapping")
        .to_ne_bytes()[at % WORD]
}

/// Checks that the byte at any offset of the mapping is the one `expected` gives for it.
fn check_any_byte(words: &Words, expected: impl Fn(usize) -> u8) {
    let at: usize = kani::any();
    if at < bytes(words) {
        assert_eq!(byte(words, at), expected(at));
    }
}

fn counts_the_words_mapped(count: usize) {
    assert_eq!(Words::new(HeapMapping::new(count)).count(), count as u64);
}

fn loads_stores_and_swaps_only_words_inside(count: usize) {
    let (words, before) = any_words(count);
    let (index, value): (u64, u64) = (kani::any(), kani::any());
    let inside = index < words.count();
    assert_eq!(words.load(index).is_ok(), inside);
    assert_eq!(words.store(index, value).is_ok(), inside);
    assert_eq!(words.swap(index, !value).ok(), inside.then_some(value));
    if inside {
        assert_eq!(words.load(index), Ok(!value));
    }
    check_any_byte(&words, |at| {
        if inside && (at / WORD) as u64 == index {
            (!value).to_ne_bytes()[at % WORD]
        } else {
            before[at]
        }
    });
}

fn reads_exactly_the_bytes_asked(count: usize) {
    let (words, before) = any_words(count);
    let at: u64 = kani::any();
    let length = kani::any_where(|length: &usize| *length <= BYTES + 1);
    let original: [u8; BYTES + 1] = kani::any();
    let mut buffer = original;
    let inside = u128::from(at) + length as u128 <= bytes(&words) as u128;
    kani::cover!(
        inside && at % WORD as u64 == 3 && at + length as u64 == bytes(&words) as u64 - 1,
        "a read from a word's fourth byte to the last word's seventh"
    );
    assert_eq!(
        words.read(at, &mut buffer[..length]),
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
    check_any_byte(&words, |at| before[at]);
}

fn writes_exactly_the_bytes_given(count: usize) {
    let (words, before) = any_words(count);
    let at: u64 = kani::any();
    let length = kani::any_where(|length: &usize| *length <= BYTES + 1);
    let written: [u8; BYTES + 1] = kani::any();
    let inside = u128::from(at) + length as u128 <= bytes(&words) as u128;
    kani::cover!(
        inside && at % WORD as u64 == 3 && at + length as u64 == bytes(&words) as u64 - 1,
        "a write from a word's fourth byte to the last word's seventh"
    );
    assert_eq!(
        words.write(at, &written[..length]),
        if inside { Ok(()) } else { Err(OutsideError) }
    );
    let first = at as usize;
    check_any_byte(&words, |byte| {
        if inside && (first..first + length).contains(&byte) {
            written[byte - first]
        } else {
            before[byte]
        }
    });
}

fn parts_lie_inside_and_reach_only_their_bytes(count: usize) {
    let (words, before) = any_words(count);
    let (start, end): (u64, u64) = (kani::any(), kani::any());
    let part = words.part(start..end);
    assert_eq!(part.is_ok(), start <= end && end <= bytes(&words) as u64);
    let Ok(part) = part else { return };
    kani::cover!(
        start % WORD as u64 == 3 && end == bytes(&words) as u64 - 1,
        "a part of a head, whole words and a tail"
    );
    let start = start as usize;
    assert_eq!(part.len(), end - start as u64);
    let length = kani::any_where(|length: &usize| *length <= BYTES);
    let count = length.min(part.len() as usize);
    let mut buffer = [0; BYTES];
    assert_eq!(part.read(&mut buffer[..length]), count);
    let index: usize = kani::any();
    if index < count {
        assert_eq!(buffer[index], before[start + index]);
    }
    let written: [u8; BYTES] = kani::any();
    assert_eq!(part.write(&written[..length]), count);
    check_any_byte(&words, |at| {
        if (start..start + count).contains(&at) {
            written[at - start]
        } else {
            before[at]
        }
    });
}

fn parts_read_to_a_sink_and_write_from_a_source(count: usize) {
    let (words, before) = any_words(count);
    let (start, end) = (
        kani::any_where(|start: &usize| *start <= BYTES),
        kani::any_where(|end: &usize| *end <= BYTES),
    );
    kani::assume(start <= end && end <= bytes(&words));
    let part = words
        .part(start as u64..end as u64)
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
    check_any_byte(&words, |at| {
        if (start..start + count).contains(&at) {
            written[at - start]
        } else {
            before[at]
        }
    });
}

/// The kernel's calls, replaced by ones that read the 4 bytes they are given, as the kernel does, so that Kani checks
/// the address lies inside the mapping, and that report what they read.
#[cfg(target_os = "linux")]
mod futex {
    use core::sync::atomic::{AtomicU32, Ordering};

    use rustix::io::{Errno, Result};
    use rustix::thread::futex::{Flags, Timespec};

    pub fn wait(half: &AtomicU32, _: Flags, expected: u32, _: Option<&Timespec>) -> Result<()> {
        if half.load(Ordering::Relaxed) == expected {
            Err(Errno::TIMEDOUT)
        } else {
            Err(Errno::AGAIN)
        }
    }

    /// Reports the 4 bytes it read as the number woken.
    pub fn wake(half: &AtomicU32, _: Flags, _: u32) -> Result<usize> {
        Ok(half.load(Ordering::Relaxed) as usize)
    }
}

#[cfg(target_os = "linux")]
fn waits_and_wakes_on_the_low_half_of_words_inside(count: usize) {
    use crate::WaitError;

    let (words, before) = any_words(count);
    let index: u64 = kani::any();
    let (expected, count): (u32, u32) = (kani::any(), kani::any());
    let inside = index < words.count();
    let outside = Err(WaitError::Outside(OutsideError));
    assert_eq!(
        words.wait(index, expected, None),
        if inside { Ok(()) } else { outside }
    );
    let woken = words.wake(index, count);
    if inside {
        let at = index as usize * WORD
            + if cfg!(target_endian = "big") {
                WORD / 2
            } else {
                0
            };
        let mut low = [0; WORD / 2];
        low.copy_from_slice(&before[at..at + WORD / 2]);
        assert_eq!(woken, Ok(u32::from_ne_bytes(low)));
    } else {
        assert_eq!(woken, Err(WaitError::Outside(OutsideError)));
    }
}

/// Makes one harness of each check above for each count of words.
macro_rules! harnesses {
    ($($count:literal => $module:ident),*) => {$(
        mod $module {
            #[kani::proof]
            #[kani::unwind(6)]
            fn counts_the_words_mapped() {
                super::counts_the_words_mapped($count);
            }

            #[kani::proof]
            #[kani::unwind(6)]
            fn loads_stores_and_swaps_only_words_inside() {
                super::loads_stores_and_swaps_only_words_inside($count);
            }

            #[kani::proof]
            #[kani::unwind(6)]
            fn reads_exactly_the_bytes_asked() {
                super::reads_exactly_the_bytes_asked($count);
            }

            #[kani::proof]
            #[kani::unwind(6)]
            fn writes_exactly_the_bytes_given() {
                super::writes_exactly_the_bytes_given($count);
            }

            #[kani::proof]
            #[kani::unwind(6)]
            fn parts_lie_inside_and_reach_only_their_bytes() {
                super::parts_lie_inside_and_reach_only_their_bytes($count);
            }

            #[kani::proof]
            #[kani::unwind(6)]
            fn parts_read_to_a_sink_and_write_from_a_source() {
                super::parts_read_to_a_sink_and_write_from_a_source($count);
            }

            #[cfg(target_os = "linux")]
            #[kani::proof]
            #[kani::unwind(6)]
            #[kani::stub(rustix::thread::futex::wait, super::futex::wait)]
            #[kani::stub(rustix::thread::futex::wake, super::futex::wake)]
            fn waits_and_wakes_on_the_low_half_of_words_inside() {
                super::waits_and_wakes_on_the_low_half_of_words_inside($count);
            }
        }
    )*};
}

harnesses!(1 => one_word, 2 => two_words, 3 => three_words, 4 => four_words);
