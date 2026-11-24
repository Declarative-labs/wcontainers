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

/// The lengths of the copies at each boundary: none, each up to two words and a byte, and around each length at which a
/// C library's copy changes how it moves bytes, up to a page and a byte.
const LENGTHS: [usize; 42] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 23, 24, 25, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 511,
    512, 513, 1023, 1024, 1025, 2047, 2048, 2049, 4095, 4096, 4097, 8191, 8193,
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

/// Checks that each word of both sizes loads what was stored, that each 8-byte word swaps out what it held, and that each
/// 4-byte word is exchanged only when it holds the value compared, all in the bytes the word covers.
pub fn loads_stores_swaps_and_exchanges_words<M: SharedMemory>(memory: M) {
    let length = bytes(&memory);
    memory
        .write(0, &vec![u8::MIN; length])
        .expect("the bytes lie inside the memory");
    check_words(&memory, 0x0123_4567_89ab_cdef_u64, u64::to_ne_bytes, |at, value| {
        assert_eq!(memory.swap(at, !value), Ok(value), "{at}");
        assert_eq!(memory.load::<u64>(at), Ok(!value), "{at}");
        assert_eq!(memory.swap(at, u64::MIN), Ok(!value), "{at}");
    });
    check_words(&memory, 0x8967_4523_u32, u32::to_ne_bytes, |at, value| {
        assert_eq!(
            memory.compare_exchange(at, !value, u32::MIN),
            Ok(value),
            "a compare-and-exchange that does not hold the value compared, at {at}"
        );
        assert_eq!(memory.load::<u32>(at), Ok(value), "{at}");
        assert_eq!(memory.compare_exchange(at, value, u32::MIN), Ok(value), "{at}");
    });
}

/// Checks the words of one size over the first words the suite covers, each starting from zero; `change` gets each word
/// holding its value and must leave it zero.
fn check_words<M: SharedMemory, W: Word + core::ops::Not<Output = W>, const SIZE: usize>(
    memory: &M,
    seed: W,
    to_bytes: fn(W) -> [u8; SIZE],
    change: impl Fn(u64, W),
) {
    for at in (0..4 * WORD).step_by(SIZE) {
        let value = if (at / SIZE).is_multiple_of(2) {
            seed
        } else {
            !seed
        };
        let zero = W::try_from(u64::MIN).unwrap_or_else(|_| unreachable!("zero fits every word"));
        assert_eq!(memory.load::<W>(at as u64), Ok(zero), "{at}");
        assert_eq!(memory.store(at as u64, value), Ok(()));
        assert_eq!(memory.load::<W>(at as u64), Ok(value), "{at}");
        assert_eq!(read_at(memory, at, SIZE), to_bytes(value), "{at}");
        change(at as u64, value);
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
            assert_eq!(each_8_byte_word_method(&memory, at), [outside; 3], "{at}");
        }
        if at > length - HALF as u64 {
            assert_eq!(each_4_byte_word_method(&memory, at), [outside; 3], "{at}");
        }
    }
    let misaligned = Err(WordError::Misaligned);
    for at in 1..WORD as u64 {
        assert_eq!(
            each_8_byte_word_method(&memory, at),
            [misaligned; 3],
            "{at}"
        );
        if at % HALF as u64 != 0 {
            assert_eq!(
                each_4_byte_word_method(&memory, at),
                [misaligned; 3],
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

/// Returns what load, store and swap of an 8-byte word at the offset return, without the value.
fn each_8_byte_word_method<M: SharedMemory>(memory: &M, at: u64) -> [Result<(), WordError>; 3] {
    [
        memory.load::<u64>(at).map(|_| ()),
        memory.store(at, u64::MAX),
        memory.swap(at, u64::MAX).map(|_| ()),
    ]
}

/// Returns what load, store and compare-and-exchange of a 4-byte word at the offset return, without the value.
fn each_4_byte_word_method<M: SharedMemory>(memory: &M, at: u64) -> [Result<(), WordError>; 3] {
    [
        memory.load::<u32>(at).map(|_| ()),
        memory.store(at, u32::MAX),
        memory.compare_exchange(at, u32::MIN, u32::MAX).map(|_| ()),
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

/// Loads, stores, swaps and compares and exchanges words of the first `length` bytes, storing only values whose bytes are
/// each the background or its complement, as the writer does; checks that each byte every method returns is one of the
/// two, and returns the number of rounds.
pub fn use_words_checking_values_held<M: SharedMemory>(memory: &M, length: usize, rounds: usize) -> usize {
    let held = |at: usize, value: &[u8]| {
        value
            .iter()
            .enumerate()
            .all(|(index, &byte)| either(at + index, byte))
    };
    let pattern = |at: usize, round: usize| -> [u8; WORD] {
        core::array::from_fn(|index| {
            if (round + index).is_multiple_of(2) {
                background(at + index)
            } else {
                !background(at + index)
            }
        })
    };
    let words = length / WORD * WORD;
    for round in 0..rounds {
        let long = (round * WORD) % words;
        let short = long + HALF * (round % 2);
        let mut half = [u8::MIN; HALF];
        half.copy_from_slice(&pattern(short, round)[..HALF]);
        match round % 4 {
            0 => {
                let value = memory.load::<u64>(long as u64).expect("inside");
                assert!(held(long, &value.to_ne_bytes()), "loaded {value:#x} at {long}");
                let value = memory.load::<u32>(short as u64).expect("inside");
                assert!(held(short, &value.to_ne_bytes()), "loaded {value:#x} at {short}");
            }
            1 => {
                memory
                    .store(long as u64, u64::from_ne_bytes(pattern(long, round)))
                    .expect("inside");
                memory
                    .store(short as u64, u32::from_ne_bytes(half))
                    .expect("inside");
            }
            2 => {
                let value = memory
                    .swap(long as u64, u64::from_ne_bytes(pattern(long, round)))
                    .expect("inside");
                assert!(held(long, &value.to_ne_bytes()), "swapped out {value:#x} at {long}");
            }
            _ => {
                let seen = memory.load::<u32>(short as u64).expect("inside");
                let value = memory
                    .compare_exchange(short as u64, seen, u32::from_ne_bytes(half))
                    .expect("inside");
                assert!(held(short, &value.to_ne_bytes()), "exchanged out {value:#x} at {short}");
            }
        }
    }
    rounds
}

/// Checks that each byte every word method returns is a value that byte held, while another thread writes the same bytes
/// with single bytes, 4-byte words and 8-byte words.
pub fn uses_words_holding_only_values_held_while_another_thread_writes_mixed_sizes<
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
        use_words_checking_values_held(&memory, length, ROUNDS);
        done.store(true, std::sync::atomic::Ordering::Relaxed);
    });
}

/// The bytes the other party owns at each end of a slot, where the copies own the bytes between: an 8-byte word, a
/// 4-byte word and a byte at the start, and the same in the opposite order at the end.
pub const EDGE: usize = WORD + HALF + 1;

/// Returns the bytes of each slot for a memory of the length: half of all but its last two words, which stay the test's
/// own, rounded down to a multiple of a word.
pub fn slot_bytes(length: usize) -> usize {
    (length - 2 * WORD) / 2 / WORD * WORD
}

/// Writes the other party's bytes at both ends of each of the two slots over and over, with 8-byte, 4-byte and
/// single-byte stores, checking before each write that the bytes still hold what it last wrote, until `done` returns
/// true and at least one check is made; returns the number of checks.
pub fn write_own_edges<M: SharedMemory>(memory: &M, done: impl Fn() -> bool) -> usize {
    let slot = slot_bytes(bytes(memory));
    let starts = [0, slot];
    let ends = starts.map(|start| start + slot);
    let mut round = 0_u64;
    let mut checks = 0;
    while round < 2 || !done() {
        for (&start, &end) in starts.iter().zip(&ends) {
            if round > 0 {
                let last = round - 1;
                assert_eq!(memory.load::<u64>(start as u64), Ok(last), "the word at {start}");
                assert_eq!(memory.load::<u64>((end - WORD) as u64), Ok(last), "the word before {end}");
                assert_eq!(memory.load::<u32>((start + WORD) as u64), Ok(last as u32), "after {start}");
                assert_eq!(memory.load::<u32>((end - EDGE + 1) as u64), Ok(last as u32), "before {end}");
                let mut byte = [u8::MIN];
                for at in [start + EDGE - 1, end - EDGE] {
                    memory.read(at as u64, &mut byte).expect("inside");
                    assert_eq!(byte, [last as u8], "the byte at {at}");
                }
                checks += 1;
            }
            memory.store(start as u64, round).expect("inside");
            memory.store((end - WORD) as u64, round).expect("inside");
            memory.store((start + WORD) as u64, round as u32).expect("inside");
            memory.store((end - EDGE + 1) as u64, round as u32).expect("inside");
            for at in [start + EDGE - 1, end - EDGE] {
                memory.write(at as u64, &[round as u8]).expect("inside");
            }
        }
        round += 1;
    }
    checks
}

/// Copies into the bytes between the other party's in the two slots, from every start within a word of the edge and at
/// lengths from one byte to all of them, and checks that each copy reads back; returns the number of bytes copied.
pub fn write_between_edges<M: SharedMemory>(memory: &M, rounds: usize) -> usize {
    let slot = slot_bytes(bytes(memory));
    let mut copied = 0;
    for round in 0..rounds {
        let first = (round % 2) * slot + EDGE + (round / 2) % WORD;
        let room = (round % 2) * slot + slot - EDGE - first;
        let count = 1 + (round * 997) % room;
        let written: Vec<u8> = (0..count)
            .map(|index| (round as u8).wrapping_mul(13).wrapping_add(index as u8))
            .collect();
        memory.write(first as u64, &written).expect("inside");
        assert_eq!(read_at(memory, first, count), written, "{count} bytes at {first}");
        copied += count;
    }
    copied
}

/// Checks that copies in change no byte beside them, while another thread writes the bytes beside them with 8-byte,
/// 4-byte and single-byte stores and checks each.
pub fn copies_in_keeping_the_bytes_beside_them_that_another_thread_writes<M: SharedMemory + Sync>(
    memory: M,
) {
    use std::sync::atomic::{AtomicBool, Ordering};

    let (started, done) = (AtomicBool::new(false), AtomicBool::new(false));
    let checks = thread::scope(|scope| {
        let writer = scope.spawn(|| {
            write_own_edges(&memory, || {
                started.store(true, Ordering::Relaxed);
                done.load(Ordering::Relaxed)
            })
        });
        while !started.load(Ordering::Relaxed) {
            thread::yield_now();
        }
        write_between_edges(&memory, ROUNDS);
        done.store(true, Ordering::Relaxed);
        writer.join().expect("the other thread finishes")
    });
    assert!(checks > 0, "the other thread never checked its bytes");
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

/// The offset of the 4-byte word the counting tests add to.
pub const COUNTED: u64 = WORD as u64;

/// The offset of the 8-byte word the counting tests swap tokens through.
pub const SWAPPED: u64 = 2 * WORD as u64;

/// Checks that compare-and-exchange and swap are each one atomic access: threads that count with them lose no count,
/// and the values swapped out are exactly the values swapped in.
pub fn counts_and_swaps_without_losing_a_value_across_threads<M: SharedMemory + Sync>(memory: M) {
    /// The threads that count and swap at once.
    const THREADS: usize = 8;
    bytes(&memory);
    let increments = ROUNDS;
    assert_eq!(memory.store(COUNTED, u32::MIN), Ok(()));
    assert_eq!(memory.store(SWAPPED, u64::MIN), Ok(()));
    let start = std::sync::Barrier::new(THREADS);
    let swapped_out: Vec<u64> = thread::scope(|scope| {
        let counters: Vec<_> = (0..THREADS)
            .map(|thread| {
                let (memory, start) = (&memory, &start);
                scope.spawn(move || {
                    start.wait();
                    count_and_swap(memory, (thread * increments) as u64, increments)
                })
            })
            .collect();
        counters
            .into_iter()
            .flat_map(|handle| handle.join().expect("the thread finishes"))
            .collect()
    });
    check_counted_and_swapped(&memory, swapped_out, (THREADS * increments) as u64);
}

/// Swaps the tokens from `first + 1` to `first + rounds` in turn into the 8-byte word at `SWAPPED`, then adds one to the
/// 4-byte word at `COUNTED` `rounds` times; returns the values swapped out.
pub fn count_and_swap<M: SharedMemory>(memory: &M, first: u64, rounds: usize) -> Vec<u64> {
    let swapped_out = (1..=rounds as u64)
        .map(|round| {
            memory
                .swap(SWAPPED, first + round)
                .expect("the word lies inside the memory")
        })
        .collect();
    (0..rounds).for_each(|_| increment(memory, COUNTED));
    swapped_out
}

/// Checks that the count is the total, and that the values swapped out and the one left are each token from 0 to the
/// total once.
pub fn check_counted_and_swapped<M: SharedMemory>(memory: &M, swapped_out: Vec<u64>, total: u64) {
    assert_eq!(memory.load::<u32>(COUNTED), Ok(total as u32));
    let mut seen = swapped_out;
    seen.push(
        memory
            .load::<u64>(SWAPPED)
            .expect("the word lies inside the memory"),
    );
    seen.sort_unstable();
    assert_eq!(seen, (0..=total).collect::<Vec<u64>>());
}

/// Adds one to the 4-byte word with a compare-and-exchange.
fn increment<M: SharedMemory>(memory: &M, at: u64) {
    let mut seen: u32 = memory.load(at).expect("the word lies inside the memory");
    loop {
        let held = memory
            .compare_exchange(at, seen, seen.wrapping_add(1))
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
            ignore = "under Miri the memory is a Rust allocation, where threads racing is a data race; native only"
        )]
        fn reads_only_values_bytes_held_while_another_thread_writes_mixed_sizes() {
            $crate::reads_only_values_bytes_held_while_another_thread_writes_mixed_sizes($memory);
        }

        #[test]
        #[cfg_attr(
            miri,
            ignore = "under Miri the memory is a Rust allocation, where threads racing is a data race; native only"
        )]
        fn uses_words_holding_only_values_held_while_another_thread_writes_mixed_sizes() {
            $crate::uses_words_holding_only_values_held_while_another_thread_writes_mixed_sizes($memory);
        }

        #[test]
        #[cfg_attr(
            miri,
            ignore = "under Miri the memory is a Rust allocation, where threads racing is a data race; native only"
        )]
        fn copies_in_keeping_the_bytes_beside_them_that_another_thread_writes() {
            $crate::copies_in_keeping_the_bytes_beside_them_that_another_thread_writes($memory);
        }

        #[test]
        #[cfg_attr(
            miri,
            ignore = "under Miri the memory is a Rust allocation, where threads racing is a data race; native only"
        )]
        fn keeps_the_bytes_beside_it_that_another_thread_writes() {
            $crate::keeps_the_bytes_beside_it_that_another_thread_writes($memory);
        }

        #[test]
        #[cfg_attr(
            miri,
            ignore = "under Miri the memory is a Rust allocation, where threads racing is a data race; native only"
        )]
        fn counts_and_swaps_without_losing_a_value_across_threads() {
            $crate::counts_and_swaps_without_losing_a_value_across_threads($memory);
        }

        #[cfg(target_os = "linux")]
        #[test]
        #[cfg_attr(
            miri,
            ignore = "under Miri the memory is a Rust allocation, where threads racing is a data race; native only"
        )]
        fn waits_until_woken() {
            $crate::waits_until_woken($memory);
        }
    };
}
