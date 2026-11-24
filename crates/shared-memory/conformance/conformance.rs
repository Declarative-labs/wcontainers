//! The tests every implementation of wcontainers-shared-memory runs, through `SharedMemory` alone.

use std::thread;
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use wcontainers_shared_memory::WaitError;
use wcontainers_shared_memory::{OutsideError, SharedMemory, Word, WordError};
use wcontainers_volatile_bytes::VolatileBytes;

/// The bytes of an 8-byte word.
const WORD: usize = size_of::<u64>();

/// The bytes of a 4-byte word.
const HALF: usize = size_of::<u32>();

/// The bytes of a cache line on the hosts these tests run on.
pub const CACHE_LINE: usize = 64;

/// The bytes the tests need the memory to hold: four words they cover in every way, and one after them that must not
/// change.
pub const NEEDED: u64 = 5 * WORD as u64;

/// The rounds of the tests where another thread writes.
const ROUNDS: usize = if cfg!(miri) { 64 } else { 20_000 };

/// The lengths of the copies at each boundary: none, each up to two words and a byte, and around a cache line and two.
const LENGTHS: [usize; 24] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 23, 24, 25, 31, 32, 33, 63, 64, 65, 127, 129,
];

/// Returns the number of bytes of the memory, which must hold the bytes the tests need.
fn bytes<M: SharedMemory>(memory: &M) -> usize {
    assert!(
        memory.len() >= NEEDED,
        "the suite needs a memory of at least {NEEDED} bytes"
    );
    usize::try_from(memory.len()).expect("the memory fits in this host's addresses")
}

/// Returns the bytes from the offset.
fn read_at<M: SharedMemory>(memory: &M, at: usize, length: usize) -> Vec<u8> {
    let mut read = vec![u8::MIN; length];
    memory
        .read(at as u64, &mut read)
        .expect("the bytes lie inside the memory");
    read
}

/// Returns the byte the tests put at the offset when it is not being changed.
pub fn background(at: usize) -> u8 {
    (at as u8).wrapping_mul(0x9d) ^ 0x5a
}

/// Checks that a part of the whole memory is as long as the memory.
pub fn counts_its_bytes<M: SharedMemory>(memory: M) {
    let length = bytes(&memory) as u64;
    assert_eq!(memory.borrow(0..length).map(|part| part.len()), Ok(length));
}

/// Checks that each word of both sizes loads what was stored, swaps out what it held, and is exchanged only when it
/// holds the value compared, all in the bytes the word covers.
pub fn loads_stores_swaps_and_exchanges_words<M: SharedMemory>(memory: M) {
    let length = bytes(&memory);
    memory
        .write(0, &vec![u8::MIN; length])
        .expect("the bytes lie inside the memory");
    check_words(&memory, 0x0123_4567_89ab_cdef_u64, u64::to_ne_bytes);
    check_words(&memory, 0x8967_4523_u32, u32::to_ne_bytes);
}

/// Checks the words of one size over the first words the suite covers, each starting from zero.
fn check_words<M: SharedMemory, W: Word + core::ops::Not<Output = W>, const SIZE: usize>(
    memory: &M,
    seed: W,
    to_bytes: fn(W) -> [u8; SIZE],
) {
    for at in (0..4 * WORD).step_by(SIZE) {
        let value = if (at / SIZE).is_multiple_of(2) {
            seed
        } else {
            !seed
        };
        let zero = word::<W>(u64::MIN);
        assert_eq!(memory.load::<W>(at as u64), Ok(zero), "{at}");
        assert_eq!(memory.store(at as u64, value), Ok(()));
        assert_eq!(memory.load::<W>(at as u64), Ok(value), "{at}");
        assert_eq!(read_at(memory, at, SIZE), to_bytes(value), "{at}");
        assert_eq!(memory.swap(at as u64, !value), Ok(value), "{at}");
        assert_eq!(memory.load::<W>(at as u64), Ok(!value), "{at}");
        assert_eq!(
            memory.compare_exchange(at as u64, value, zero),
            Ok(!value),
            "a compare-and-exchange that does not hold the value compared, at {at}"
        );
        assert_eq!(memory.load::<W>(at as u64), Ok(!value), "{at}");
        assert_eq!(
            memory.compare_exchange(at as u64, !value, zero),
            Ok(!value),
            "{at}"
        );
        assert_eq!(memory.load::<W>(at as u64), Ok(zero), "{at}");
        assert!(
            read_at(memory, 0, 5 * WORD)
                .iter()
                .all(|&byte| byte == u8::MIN),
            "every word went back to zero, at {at}"
        );
    }
}

/// Checks that a write at every offset and length of the first words reads back, and changes no other byte.
pub fn writes_and_reads_back_at_every_offset_and_length<M: SharedMemory>(memory: M) {
    bytes(&memory);
    let covered = 4 * WORD;
    let mut model: Vec<u8> = (0..covered + WORD).map(background).collect();
    assert_eq!(memory.write(0, &model), Ok(()));
    let mut stamp = u8::MIN;
    for at in 0..=covered {
        for length in 0..=covered - at {
            stamp = stamp.wrapping_add(1);
            let written: Vec<u8> = (0..length)
                .map(|index| stamp.wrapping_add(index as u8).wrapping_mul(7))
                .collect();
            assert_eq!(memory.write(at as u64, &written), Ok(()));
            model[at..at + length].copy_from_slice(&written);
            assert_eq!(
                read_at(&memory, at, length),
                written,
                "{length} bytes at {at}"
            );
            assert_eq!(
                read_at(&memory, 0, model.len()),
                model,
                "after {length} bytes at {at}"
            );
        }
    }
}

/// Checks copies in both directions at every offset within two words and a byte of each word, cache-line and page
/// boundary the memory holds and of its end, at lengths around those sizes, from and into buffers at every alignment:
/// each copies exactly its bytes, and changes no other byte of the memory or of the buffer.
pub fn copies_exactly_around_word_cache_line_and_page_boundaries<M: SharedMemory>(
    memory: M,
    page: usize,
) {
    /// The offsets either side of a boundary that the copies start at.
    const SPAN: usize = 2 * WORD + 1;
    /// The byte a buffer holds outside the bytes copied.
    const SENTINEL: u8 = 0xee;

    let length = bytes(&memory);
    let mut model: Vec<u8> = (0..length).map(background).collect();
    assert_eq!(memory.write(0, &model), Ok(()));
    let boundaries = [WORD, CACHE_LINE, page, 2 * page, length];
    let mut stamp = u8::MIN;
    let mut cases = 0_u64;
    for &boundary in boundaries.iter().filter(|&&boundary| boundary <= length) {
        let starts = (boundary.saturating_sub(SPAN)..=boundary + SPAN)
            .flat_map(|start| LENGTHS.map(move |count| (start, count)))
            .chain(
                LENGTHS
                    .iter()
                    .filter(|&&count| count <= boundary)
                    .map(|&count| (boundary - count, count)),
            );
        for (start, count) in starts.filter(|&(start, count)| start + count <= length) {
            cases += 1;
            stamp = stamp.wrapping_add(1);
            let shift = (start + count) % WORD;
            let written: Vec<u8> = (0..count)
                .map(|index| stamp.wrapping_mul(31).wrapping_add(index as u8) | 1)
                .collect();
            let mut source = vec![SENTINEL; count + 2 * WORD];
            source[shift..shift + count].copy_from_slice(&written);
            assert_eq!(
                memory.write(start as u64, &source[shift..shift + count]),
                Ok(())
            );
            model[start..start + count].copy_from_slice(&written);

            let mut into = vec![SENTINEL; count + 2 * WORD];
            assert_eq!(
                memory.read(start as u64, &mut into[shift..shift + count]),
                Ok(())
            );
            assert_eq!(
                into[shift..shift + count],
                written,
                "{count} bytes at {start}"
            );
            assert!(
                into[..shift]
                    .iter()
                    .chain(&into[shift + count..])
                    .all(|&byte| byte == SENTINEL),
                "a read of {count} bytes at {start} wrote past its buffer"
            );
            let window = start.saturating_sub(SPAN)..(start + count + SPAN).min(length);
            assert_eq!(
                read_at(&memory, window.start, window.len()),
                model[window.clone()],
                "a write of {count} bytes at {start} changed another byte"
            );
        }
    }
    assert_eq!(read_at(&memory, 0, length), model);
    assert!(cases > 0);
}

/// Checks that offsets past the end, past the largest offset or not a multiple of a word's size are refused, and change
/// neither the memory nor the buffer.
pub fn refuses_offsets_outside<M: SharedMemory>(memory: M) {
    let length = bytes(&memory) as u64;
    let outside = Err(WordError::Outside(OutsideError));
    for at in [length, length - 4, length + 8, u64::MAX - 7, u64::MAX - 3] {
        if at > length - WORD as u64 {
            assert_eq!(
                each_word_method::<M, u64>(&memory, at),
                [outside; 4],
                "{at}"
            );
        }
        if at > length - HALF as u64 {
            assert_eq!(
                each_word_method::<M, u32>(&memory, at),
                [outside; 4],
                "{at}"
            );
        }
    }
    let misaligned = Err(WordError::Misaligned);
    for at in 1..WORD as u64 {
        assert_eq!(
            each_word_method::<M, u64>(&memory, at),
            [misaligned; 4],
            "{at}"
        );
        if at % HALF as u64 != 0 {
            assert_eq!(
                each_word_method::<M, u32>(&memory, at),
                [misaligned; 4],
                "{at}"
            );
        }
    }
    assert_eq!(memory.write(length - 2, &[1, 2]), Ok(()));
    for (at, count) in [
        (length - 1, 2),
        (length, 1),
        (length + 1, 0),
        (u64::MAX, 1),
        (u64::MAX - 2, WORD),
        (u64::MAX, 0),
    ] {
        let mut buffer = vec![u8::MAX; count];
        assert_eq!(
            memory.read(at, &mut buffer),
            Err(OutsideError),
            "{count} bytes at {at}"
        );
        assert_eq!(buffer, vec![u8::MAX; count]);
        assert_eq!(
            memory.write(at, &buffer),
            Err(OutsideError),
            "{count} bytes at {at}"
        );
        assert_eq!(read_at(&memory, length as usize - 2, 2), [1, 2]);
    }
    assert!(
        read_at(&memory, 0, WORD)
            .iter()
            .all(|&byte| byte == u8::MIN)
    );
    assert_eq!(memory.read(length, &mut []), Ok(()));
    assert_eq!(memory.write(length, &[]), Ok(()));
}

/// Returns what load, store, swap and compare-and-exchange of a word of the type at the offset return, without the value.
fn each_word_method<M: SharedMemory, W: Word>(memory: &M, at: u64) -> [Result<(), WordError>; 4] {
    let all = word::<W>(u64::from(u32::MAX));
    [
        memory.load::<W>(at).map(|_| ()),
        memory.store(at, all),
        memory.swap(at, all).map(|_| ()),
        memory.compare_exchange(at, word(u64::MIN), all).map(|_| ()),
    ]
}

/// Checks that a part lies inside the memory, and reads and writes its own bytes.
pub fn parts_stay_inside<M: SharedMemory>(memory: M) {
    let length = bytes(&memory) as u64;
    for refused in [
        0..length + 1,
        length..length - 1,
        length + 1..length + 1,
        u64::MAX..u64::MAX,
        1..u64::MAX,
    ] {
        assert!(memory.borrow(refused.clone()).is_err(), "{refused:?}");
    }
    assert_eq!(memory.borrow(length..length).map(|part| part.len()), Ok(0));
    assert_eq!(memory.write(0, &[u8::MIN; 2 * WORD]), Ok(()));
    let part = memory
        .borrow(3..3 + WORD as u64)
        .expect("the offsets lie inside the memory");
    assert_eq!(part.write(&[u8::MAX; 2 * WORD]), WORD);
    let mut expected = [u8::MIN; 2 * WORD];
    expected[3..3 + WORD].fill(u8::MAX);
    assert_eq!(read_at(&memory, 0, 2 * WORD), expected);
}

/// The two values each byte takes while another party writes: the background, or its complement.
fn either(at: usize, byte: u8) -> bool {
    byte == background(at) || byte == !background(at)
}

/// Writes the bytes from the offset in turn with single bytes, 4-byte words and 8-byte words, each byte the background
/// or its complement, until `done` returns true.
pub fn write_mixed_sizes<M: SharedMemory>(memory: &M, length: usize, done: impl Fn() -> bool) {
    let flip = |at: usize, round: usize| {
        if (at / WORD + round).is_multiple_of(3) {
            !background(at)
        } else {
            background(at)
        }
    };
    let mut round = 0;
    while !done() {
        round += 1;
        for at in (0..length).step_by(WORD) {
            let mut word = [u8::MIN; WORD];
            for (index, byte) in word.iter_mut().enumerate() {
                *byte = flip(at + index, round + index);
            }
            match (at / WORD + round) % 3 {
                0 => {
                    for (index, &byte) in word.iter().enumerate() {
                        memory
                            .write((at + index) as u64, &[byte])
                            .expect("the byte lies inside the memory");
                    }
                }
                1 => {
                    for half in 0..WORD / HALF {
                        let mut value = [u8::MIN; HALF];
                        value.copy_from_slice(&word[half * HALF..(half + 1) * HALF]);
                        memory
                            .store((at + half * HALF) as u64, u32::from_ne_bytes(value))
                            .expect("the word lies inside the memory");
                    }
                }
                _ => memory
                    .store(at as u64, u64::from_ne_bytes(word))
                    .expect("the word lies inside the memory"),
            }
        }
    }
}

/// Checks reads at every alignment and many lengths of the first `length` bytes: each byte is the background or its
/// complement, the two values the writer stores; returns the number of bytes checked.
pub fn read_values_each_byte_held<M: SharedMemory>(
    memory: &M,
    length: usize,
    rounds: usize,
) -> usize {
    let mut checked = 0;
    for round in 0..rounds {
        let start = round % (2 * WORD + 1);
        let count = (round * 37) % (length - start) + 1;
        let read = read_at(memory, start, count);
        for (index, &byte) in read.iter().enumerate() {
            assert!(
                either(start + index, byte),
                "byte {} read as {byte:#x}, a value it never held",
                start + index
            );
        }
        checked += count;
    }
    checked
}

/// Checks that each byte a read copies is a value that byte held, while another thread writes the same bytes with
/// single bytes, 4-byte words and 8-byte words.
pub fn reads_only_values_bytes_held_while_another_thread_writes_mixed_sizes<
    M: SharedMemory + Sync,
>(
    memory: M,
) {
    let length = bytes(&memory).min(2 * CACHE_LINE + WORD);
    let model: Vec<u8> = (0..length).map(background).collect();
    assert_eq!(memory.write(0, &model), Ok(()));
    let done = std::sync::atomic::AtomicBool::new(false);
    thread::scope(|scope| {
        scope.spawn(|| {
            write_mixed_sizes(&memory, length, || {
                done.load(std::sync::atomic::Ordering::Relaxed)
            })
        });
        read_values_each_byte_held(&memory, length, ROUNDS);
        done.store(true, std::sync::atomic::Ordering::Relaxed);
    });
}

/// Checks that a write keeps the bytes beside it in the same word, which another thread writes meanwhile.
pub fn keeps_the_bytes_beside_it_that_another_thread_writes<M: SharedMemory + Sync>(memory: M) {
    bytes(&memory);
    assert_eq!(memory.store(0, u64::MIN), Ok(()));
    let write_own = |at: usize, count: usize, value: u8| {
        memory
            .write(at as u64, &vec![value; count])
            .expect("the bytes lie inside the memory");
        assert_eq!(
            read_at(&memory, at, count),
            vec![value; count],
            "the other thread's write kept these bytes"
        );
    };
    thread::scope(|scope| {
        scope.spawn(|| (1..=ROUNDS).for_each(|round| write_own(3, WORD - 3, round as u8)));
        (1..=ROUNDS).for_each(|round| write_own(0, 3, (round as u8).wrapping_mul(3)));
    });
    let mut expected = [ROUNDS as u8; WORD];
    expected[..3].fill((ROUNDS as u8).wrapping_mul(3));
    assert_eq!(read_at(&memory, 0, WORD), expected);
}

/// Checks that compare-and-exchange and swap are each one atomic access: threads that count with them lose no count,
/// and the values swapped out are exactly the values swapped in.
pub fn counts_and_swaps_without_losing_a_value_across_threads<M: SharedMemory + Sync>(memory: M) {
    /// The threads that count and swap at once.
    const THREADS: usize = 4;
    bytes(&memory);
    let increments = ROUNDS / 4;
    assert_eq!(memory.store(0, u64::MIN), Ok(()));
    assert_eq!(memory.store(WORD as u64, u32::MIN), Ok(()));
    assert_eq!(memory.store(2 * WORD as u64, u64::MIN), Ok(()));
    let swapped_out: Vec<Vec<u64>> = thread::scope(|scope| {
        let counters: Vec<_> = (0..THREADS)
            .map(|thread| {
                let memory = &memory;
                scope.spawn(move || {
                    let mut out = Vec::new();
                    for round in 0..increments {
                        increment::<M, u64>(memory, 0);
                        increment::<M, u32>(memory, WORD as u64);
                        let token = (thread * increments + round + 1) as u64;
                        out.push(
                            memory
                                .swap(2 * WORD as u64, token)
                                .expect("the word lies inside the memory"),
                        );
                    }
                    out
                })
            })
            .collect();
        counters
            .into_iter()
            .map(|handle| handle.join().expect("the thread finishes"))
            .collect()
    });
    let total = (THREADS * increments) as u64;
    assert_eq!(memory.load::<u64>(0), Ok(total));
    assert_eq!(memory.load::<u32>(WORD as u64), Ok(total as u32));
    let mut seen: Vec<u64> = swapped_out.into_iter().flatten().collect();
    seen.push(
        memory
            .load::<u64>(2 * WORD as u64)
            .expect("the word lies inside the memory"),
    );
    seen.sort_unstable();
    assert_eq!(seen, (0..=total).collect::<Vec<u64>>());
}

/// Returns the value as a word, which it fits.
fn word<W: Word>(value: u64) -> W {
    W::try_from(value).unwrap_or_else(|_| panic!("{value} fits the word"))
}

/// Adds one to the word with a compare-and-exchange.
fn increment<M: SharedMemory, W: Word>(memory: &M, at: u64) {
    let mut seen: W = memory.load(at).expect("the word lies inside the memory");
    loop {
        let next = word::<W>(seen.into() + 1);
        let held = memory
            .compare_exchange(at, seen, next)
            .expect("the word lies inside the memory");
        if held == seen {
            return;
        }
        seen = held;
    }
}

/// Checks that a wait returns when the word changed, when woken and when the timeout passes.
#[cfg(target_os = "linux")]
pub fn waits_until_woken<M: SharedMemory + Sync>(memory: M) {
    let length = bytes(&memory) as u64;
    let outside = WaitError::Word(WordError::Outside(OutsideError));
    assert_eq!(memory.wait(length, 0, None), Err(outside));
    assert_eq!(memory.wake(length, 1), Err(outside));
    assert_eq!(
        memory.wait(2, 0, None),
        Err(WaitError::Word(WordError::Misaligned))
    );
    let at = HALF as u64;
    assert_eq!(memory.store(at, u32::MIN), Ok(()));
    assert_eq!(memory.wake(at, u32::MAX), Ok(0));
    assert_eq!(memory.wait(at, 1, None), Ok(()));
    assert_eq!(memory.wait(at, 0, Some(Duration::from_millis(1))), Ok(()));
    thread::scope(|scope| {
        let waiter = scope.spawn(|| {
            while memory.load::<u32>(at) == Ok(0) {
                memory
                    .wait(at, 0, None)
                    .expect("the word lies inside the memory");
            }
        });
        assert_eq!(memory.store(at, 1_u32), Ok(()));
        while !waiter.is_finished() {
            memory
                .wake(at, u32::MAX)
                .expect("the word lies inside the memory");
            thread::yield_now();
        }
    });
}

/// Runs each suite function above as its own test on a memory made anew for every test; `$page` is the host's page
/// size.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($memory:expr, $page:expr) => {
        #[test]
        fn counts_its_bytes() {
            $crate::counts_its_bytes($memory);
        }

        #[test]
        fn loads_stores_swaps_and_exchanges_words() {
            $crate::loads_stores_swaps_and_exchanges_words($memory);
        }

        #[test]
        fn writes_and_reads_back_at_every_offset_and_length() {
            $crate::writes_and_reads_back_at_every_offset_and_length($memory);
        }

        #[test]
        fn copies_exactly_around_word_cache_line_and_page_boundaries() {
            $crate::copies_exactly_around_word_cache_line_and_page_boundaries($memory, $page);
        }

        #[test]
        fn refuses_offsets_outside() {
            $crate::refuses_offsets_outside($memory);
        }

        #[test]
        fn parts_stay_inside() {
            $crate::parts_stay_inside($memory);
        }

        #[test]
        #[cfg_attr(
            miri,
            ignore = "the heap's bytes are a Rust allocation, where threads racing is a data race; native only"
        )]
        fn reads_only_values_bytes_held_while_another_thread_writes_mixed_sizes() {
            $crate::reads_only_values_bytes_held_while_another_thread_writes_mixed_sizes($memory);
        }

        #[test]
        #[cfg_attr(
            miri,
            ignore = "the heap's bytes are a Rust allocation, where threads racing is a data race; native only"
        )]
        fn keeps_the_bytes_beside_it_that_another_thread_writes() {
            $crate::keeps_the_bytes_beside_it_that_another_thread_writes($memory);
        }

        #[test]
        #[cfg_attr(
            miri,
            ignore = "the heap's bytes are a Rust allocation, where threads racing is a data race; native only"
        )]
        fn counts_and_swaps_without_losing_a_value_across_threads() {
            $crate::counts_and_swaps_without_losing_a_value_across_threads($memory);
        }

        #[cfg(target_os = "linux")]
        #[test]
        #[cfg_attr(
            miri,
            ignore = "the heap's bytes are a Rust allocation, where threads racing is a data race; native only"
        )]
        fn waits_until_woken() {
            $crate::waits_until_woken($memory);
        }
    };
}
