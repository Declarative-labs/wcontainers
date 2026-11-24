//! The tests every mapping of wcontainers-shared-memory runs, through `Words` alone.

use std::thread;
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use wcontainers_shared_memory::WaitError;
use wcontainers_shared_memory::{OutsideError, Words};
use wcontainers_volatile_bytes::VolatileBytes;

/// The bytes of one word.
const WORD: usize = size_of::<u64>();

/// The words the tests need the mapping to hold: four they cover in every way, and one after them that must not change.
pub const NEEDED: u64 = 5;

/// The rounds of the tests where another thread writes; few, so that Miri finishes them.
const ROUNDS: u8 = 64;

/// Returns the number of bytes mapped, which must hold the words the tests need.
fn bytes(words: &Words) -> u64 {
    assert!(words.count() >= NEEDED, "the suite needs a mapping of at least {NEEDED} words");
    words.count() * WORD as u64
}

/// Returns the bytes from the start of the mapping.
fn read_start(words: &Words, length: usize) -> Vec<u8> {
    let mut read = vec![u8::MIN; length];
    words.read(u64::MIN, &mut read).expect("the start lies inside the mapping");
    read
}

/// Checks that the part of the whole mapping is as long as the mapping, and the count is its length in words.
pub fn counts_its_words(words: Words) {
    let bytes = bytes(&words);
    assert_eq!(words.part(0..bytes).map(|part| part.len()), Ok(bytes));
}

/// Checks that each word loads what was stored and swaps out what it held.
pub fn loads_what_was_stored(words: Words) {
    let value = |index: u64| index.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let covered = words.count().min(u64::from(u8::MAX));
    for index in 0..covered {
        assert_eq!(words.store(index, value(index)), Ok(()));
    }
    for index in 0..covered {
        assert_eq!(words.load(index), Ok(value(index)));
        assert_eq!(words.swap(index, !value(index)), Ok(value(index)));
        assert_eq!(words.load(index), Ok(!value(index)));
    }
}

/// Checks that a write at every offset and length of the first words reads back, and changes no other byte.
pub fn writes_and_reads_back_at_every_offset_and_length(words: Words) {
    bytes(&words);
    let covered = 4 * WORD;
    let mut model: Vec<u8> = (0..covered + WORD).map(|index| index as u8 ^ 0x5a).collect();
    assert_eq!(words.write(0, &model), Ok(()));
    let mut stamp = u8::MIN;
    for at in 0..=covered {
        for length in 0..=covered - at {
            stamp = stamp.wrapping_add(1);
            let written: Vec<u8> = (0..length).map(|index| stamp.wrapping_add(index as u8).wrapping_mul(7)).collect();
            assert_eq!(words.write(at as u64, &written), Ok(()));
            model[at..at + length].copy_from_slice(&written);
            let mut read = vec![u8::MIN; length];
            assert_eq!(words.read(at as u64, &mut read), Ok(()));
            assert_eq!(read, written, "{length} bytes at {at}");
            assert_eq!(read_start(&words, model.len()), model, "after {length} bytes at {at}");
        }
    }
}

/// Checks that offsets past the end or past the largest offset are refused, and change neither the mapping nor the buffer.
pub fn refuses_offsets_outside(words: Words) {
    let bytes = bytes(&words);
    for index in [words.count(), words.count() + 1, u64::MAX] {
        assert_eq!(words.load(index), Err(OutsideError));
        assert_eq!(words.store(index, u64::MAX), Err(OutsideError));
        assert_eq!(words.swap(index, u64::MAX), Err(OutsideError));
    }
    assert_eq!(words.write(bytes - 2, &[1, 2]), Ok(()));
    for (at, length) in [(bytes - 1, 2), (bytes, 1), (bytes + 1, 0), (u64::MAX, 1), (u64::MAX - 2, WORD), (u64::MAX, 0)] {
        let mut buffer = vec![u8::MAX; length];
        assert_eq!(words.read(at, &mut buffer), Err(OutsideError), "{length} bytes at {at}");
        assert_eq!(buffer, vec![u8::MAX; length]);
        assert_eq!(words.write(at, &buffer), Err(OutsideError), "{length} bytes at {at}");
        let mut last = [u8::MIN; 2];
        assert_eq!(words.read(bytes - 2, &mut last), Ok(()));
        assert_eq!(last, [1, 2]);
    }
    assert_eq!(words.read(bytes, &mut []), Ok(()));
    assert_eq!(words.write(bytes, &[]), Ok(()));
}

/// Checks that a part lies inside the mapping, and reads and writes its own bytes.
pub fn parts_stay_inside(words: Words) {
    let bytes = bytes(&words);
    for refused in [0..bytes + 1, bytes..bytes - 1, bytes + 1..bytes + 1, u64::MAX..u64::MAX, 1..u64::MAX] {
        assert!(words.part(refused.clone()).is_err(), "{refused:?}");
    }
    assert_eq!(words.part(bytes..bytes).map(|part| part.len()), Ok(0));
    assert_eq!(words.write(0, &[u8::MIN; 2 * WORD]), Ok(()));
    let part = words.part(3..3 + WORD as u64).expect("the offsets lie inside the mapping");
    assert_eq!(part.write(&[u8::MAX; 2 * WORD]), WORD);
    let mut expected = [u8::MIN; 2 * WORD];
    expected[3..3 + WORD].fill(u8::MAX);
    assert_eq!(read_start(&words, 2 * WORD), expected);
}

/// Checks that each word is read whole while another thread stores whole words.
pub fn reads_whole_words_while_another_thread_stores_them(words: Words) {
    bytes(&words);
    thread::scope(|scope| {
        scope.spawn(|| {
            for round in 0..ROUNDS {
                for index in 0..4 {
                    words.store(index, u64::from_ne_bytes([round; WORD])).expect("the word lies inside the mapping");
                }
            }
        });
        for _ in 0..ROUNDS {
            let mut read = [u8::MIN; 2 * WORD];
            words.read(4, &mut read).expect("the bytes lie inside the mapping");
            for piece in [&read[..4], &read[4..4 + WORD], &read[4 + WORD..]] {
                assert!(piece.iter().all(|&byte| byte == piece[0]), "bytes of one word read at one moment: {read:?}");
            }
        }
    });
}

/// Checks that a write of part of a word keeps the rest, which another thread writes meanwhile.
pub fn keeps_the_rest_of_a_word_another_thread_writes(words: Words) {
    bytes(&words);
    assert_eq!(words.store(0, 0), Ok(()));
    let half = WORD / 2;
    let write_half = |at: usize, value: u8| {
        words.write(at as u64, &[value; WORD / 2]).expect("the bytes lie inside the mapping");
        let mut own = [u8::MIN; WORD / 2];
        words.read(at as u64, &mut own).expect("the bytes lie inside the mapping");
        assert_eq!(own, [value; WORD / 2], "the other thread's write kept these bytes");
    };
    thread::scope(|scope| {
        scope.spawn(|| (1..=ROUNDS).for_each(|round| write_half(half, round)));
        (1..=ROUNDS).for_each(|round| write_half(0, round.wrapping_mul(3)));
    });
    let mut expected = [ROUNDS; WORD];
    expected[..half].fill(ROUNDS.wrapping_mul(3));
    assert_eq!(read_start(&words, WORD), expected);
}

/// Checks that a wait returns when the word changed, when woken and when the timeout passes.
#[cfg(target_os = "linux")]
pub fn waits_until_woken(words: Words) {
    let count = words.count();
    bytes(&words);
    assert_eq!(words.wait(count, 0, None), Err(WaitError::Outside(OutsideError)));
    assert_eq!(words.wake(count, 1), Err(WaitError::Outside(OutsideError)));
    assert_eq!(words.store(1, 0), Ok(()));
    assert_eq!(words.wake(1, u32::MAX), Ok(0));
    assert_eq!(words.wait(1, 1, None), Ok(()));
    assert_eq!(words.wait(1, 0, Some(Duration::from_millis(1))), Ok(()));
    thread::scope(|scope| {
        let waiter = scope.spawn(|| {
            while words.load(1) == Ok(0) {
                words.wait(1, 0, None).expect("the word lies inside the mapping");
            }
        });
        assert_eq!(words.store(1, 1), Ok(()));
        while !waiter.is_finished() {
            words.wake(1, u32::MAX).expect("the word lies inside the mapping");
            thread::yield_now();
        }
    });
}

/// Runs each suite function above as its own test on `Words` made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($words:expr) => {
        #[test]
        fn counts_its_words() {
            $crate::counts_its_words($words);
        }

        #[test]
        fn loads_what_was_stored() {
            $crate::loads_what_was_stored($words);
        }

        #[test]
        fn writes_and_reads_back_at_every_offset_and_length() {
            $crate::writes_and_reads_back_at_every_offset_and_length($words);
        }

        #[test]
        fn refuses_offsets_outside() {
            $crate::refuses_offsets_outside($words);
        }

        #[test]
        fn parts_stay_inside() {
            $crate::parts_stay_inside($words);
        }

        #[test]
        fn reads_whole_words_while_another_thread_stores_them() {
            $crate::reads_whole_words_while_another_thread_stores_them($words);
        }

        #[test]
        fn keeps_the_rest_of_a_word_another_thread_writes() {
            $crate::keeps_the_rest_of_a_word_another_thread_writes($words);
        }

        #[cfg(target_os = "linux")]
        #[test]
        fn waits_until_woken() {
            $crate::waits_until_woken($words);
        }
    };
}
