//! The independent check's attacks: safe callers that try to cause undefined behavior through `SharedMemory` and `Part`
//! alone, each of which must finish under Miri with no report. The heap's bytes are a Rust allocation under Miri, where
//! two threads racing is a data race by definition, so the attacks that raced threads run natively instead, in the
//! conformance suite's threaded tests over the host's mappings.
#![cfg(miri)]

use std::thread;

use wcontainers_shared_memory::{HeapMapping, OutsideError, SharedMemory, WordError};
use wcontainers_volatile_bytes::VolatileBytes;

const WORD: u64 = size_of::<u64>() as u64;

/// Two parts over the same bytes, used in turn, with writes that straddle words and edges.
#[test]
fn two_parts_of_the_same_bytes_in_turn() {
    let memory = HeapMapping::new(4);
    let a = memory.borrow(2..30).unwrap();
    let b = memory.borrow(0..32).unwrap();
    for round in 0..24u8 {
        memory.write(3, &[round; 13]).unwrap();
        let mut seen = [0; 13];
        memory.read(5, &mut seen).unwrap();
        a.write(&[round; 28]);
        assert_eq!(
            memory.swap(8, u64::from(round)),
            Ok(u64::from_ne_bytes([round; 8]))
        );
        assert_eq!(
            memory.compare_exchange(4, 0_u32, 1),
            Ok(u32::from_ne_bytes([round; 4]))
        );
        let mut whole = [0; 32];
        b.read(&mut whole);
    }
}

/// The memory moves to another thread and back.
#[test]
fn the_memory_moves_to_another_thread_and_back() {
    let memory = HeapMapping::new(2);
    let memory = thread::spawn(move || {
        memory.write(0, &[7; 16]).unwrap();
        memory
    })
    .join()
    .unwrap();
    assert_eq!(memory.load::<u64>(WORD), Ok(u64::from_ne_bytes([7; 8])));
}

/// Every offset and length near the end and near overflow: success exactly when inside, and no fault.
#[test]
fn offsets_and_lengths_at_the_edges_and_at_overflow() {
    let memory = HeapMapping::new(4);
    let bytes = 4 * WORD;
    let near_end = 0..=bytes + 9;
    let near_overflow = (u64::MAX - 40)..=u64::MAX;
    for at in near_end.chain(near_overflow) {
        for length in 0..=40usize {
            let inside = u128::from(at) + length as u128 <= u128::from(bytes);
            let mut buffer = vec![0xa5; length];
            assert_eq!(
                memory.read(at, &mut buffer).is_ok(),
                inside,
                "read {length} at {at}"
            );
            assert_eq!(
                memory.write(at, &buffer).is_ok(),
                inside,
                "write {length} at {at}"
            );
        }
        let word = |size: u64| {
            if u128::from(at) + u128::from(size) > u128::from(bytes) {
                Err(WordError::Outside(OutsideError))
            } else if at % size != 0 {
                Err(WordError::Misaligned)
            } else {
                Ok(())
            }
        };
        assert_eq!(memory.load::<u64>(at).map(|_| ()), word(WORD));
        assert_eq!(memory.store(at, 1_u32), word(4));
        assert_eq!(memory.swap(at, 1_u64).map(|_| ()), word(WORD));
        assert_eq!(memory.compare_exchange(at, 1_u32, 2).map(|_| ()), word(4));
    }
    for (start, end) in [
        (0, bytes + 1),
        (bytes, bytes - 1),
        (u64::MAX, u64::MAX),
        (1, u64::MAX),
        (u64::MAX, 0),
    ] {
        assert!(memory.borrow(start..end).is_err(), "{start}..{end}");
    }
    let tail = memory.borrow(bytes - 1..bytes).unwrap();
    assert_eq!(tail.write(&[1; 100]), 1);
    let mut seen = [0; 100];
    assert_eq!(tail.read(&mut seen), 1);
    let end = memory.borrow(bytes..bytes).unwrap();
    assert_eq!(end.write(&[1; 9]), 0);
    assert_eq!(end.read(&mut seen), 0);
}

/// A memory of no bytes, and empty parts.
#[test]
fn a_zero_length_region() {
    let memory = HeapMapping::new(0);
    assert_eq!(memory.len(), 0);
    assert!(memory.read(0, &mut []).is_ok() && memory.write(0, &[]).is_ok());
    assert!(memory.read(0, &mut [0]).is_err() && memory.write(0, &[0]).is_err());
    assert!(memory.read(u64::MAX, &mut []).is_err());
    assert!(memory.load::<u32>(0).is_err() && memory.store(0, 0_u64).is_err());
    let part = memory.borrow(0..0).unwrap();
    assert_eq!(part.read(&mut [0; 8]), 0);
    assert_eq!(part.write(&[0; 8]), 0);
    let mut sink = Vec::new();
    assert_eq!(part.read_to(&mut sink), Ok(0));
    let mut source: &[u8] = &[1, 2, 3];
    assert_eq!(part.write_from(&mut source), Ok(0));
}

/// A source that claims to have read more bytes than the buffer holds: a panic, never a write outside.
#[test]
#[should_panic]
fn a_source_that_lies_about_its_count() {
    struct Liar;
    impl wcontainers_byte_source::ByteSource for Liar {
        type Error = ();
        fn read(&mut self, into: &mut [u8]) -> Result<usize, ()> {
            Ok(into.len() + 8)
        }
    }
    let memory = HeapMapping::new(2);
    let part = memory.borrow(0..8).unwrap();
    let _ = part.write_from(&mut Liar);
}
