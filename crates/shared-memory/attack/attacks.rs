//! The independent check's attacks: safe callers that try to cause undefined behavior through `Words` and `Part`
//! alone, each of which must finish under Miri with no report.
#![cfg(miri)]

use std::thread;

use wcontainers_shared_memory::{HeapMapping, OutsideError, Words};
use wcontainers_volatile_bytes::VolatileBytes;

const WORD: u64 = 8;

/// Two views of the same bytes, used from three threads at once, with writes that straddle words and edges.
#[test]
fn two_views_of_the_same_bytes_at_once() {
    let mapping = HeapMapping::new(4);
    let a = Words::new(mapping.clone());
    let b = Words::new(mapping);
    thread::scope(|scope| {
        scope.spawn(|| {
            for round in 0..24u8 {
                a.write(3, &[round; 13]).unwrap();
                let mut seen = [0; 13];
                a.read(5, &mut seen).unwrap();
            }
        });
        scope.spawn(|| {
            for round in 0..24u8 {
                b.write(1, &[round; 20]).unwrap();
                b.swap(1, u64::from(round)).unwrap();
                b.store(3, u64::from(round)).unwrap();
            }
        });
        let pa = a.part(2..30).unwrap();
        let pb = b.part(0..32).unwrap();
        for round in 0..24u8 {
            pa.write(&[round; 28]);
            let mut seen = [0; 32];
            pb.read(&mut seen);
        }
    });
}

/// The view outlives every other handle of its mapping and moves to another thread.
#[test]
fn a_view_outlives_every_other_handle() {
    let mapping = HeapMapping::new(2);
    let words = Words::new(mapping.clone());
    drop(mapping);
    let words = thread::spawn(move || {
        words.write(0, &[7; 16]).unwrap();
        words
    })
    .join()
    .unwrap();
    assert_eq!(words.load(1), Ok(u64::from_ne_bytes([7; 8])));
}

/// A part handed to another thread while the owner of the words writes the same bytes.
#[test]
fn a_part_on_another_thread() {
    let words = Words::new(HeapMapping::new(3));
    let part = words.part(5..19).unwrap();
    thread::scope(|scope| {
        scope.spawn(|| {
            for round in 0..24u8 {
                part.write(&[round; 64]);
                let mut seen = vec![0; 64];
                part.read(&mut seen);
            }
        });
        for round in 0..24u8 {
            words.write(0, &[round; 24]).unwrap();
        }
    });
}

/// Every offset and length near the end and near overflow: success exactly when inside, and no fault.
#[test]
fn offsets_and_lengths_at_the_edges_and_at_overflow() {
    let words = Words::new(HeapMapping::new(4));
    let bytes = 4 * WORD;
    let near_end = 0..=bytes + 9;
    let near_overflow = (u64::MAX - 40)..=u64::MAX;
    for at in near_end.chain(near_overflow) {
        for length in 0..=40usize {
            let inside = u128::from(at) + length as u128 <= u128::from(bytes);
            let mut buffer = vec![0xa5; length];
            assert_eq!(
                words.read(at, &mut buffer).is_ok(),
                inside,
                "read {length} at {at}"
            );
            assert_eq!(
                words.write(at, &buffer).is_ok(),
                inside,
                "write {length} at {at}"
            );
        }
    }
    for index in [4, 5, u64::MAX / 8, u64::MAX] {
        assert_eq!(words.load(index), Err(OutsideError));
        assert_eq!(words.store(index, 1), Err(OutsideError));
        assert_eq!(words.swap(index, 1), Err(OutsideError));
    }
    for (start, end) in [
        (0, bytes + 1),
        (bytes, bytes - 1),
        (u64::MAX, u64::MAX),
        (1, u64::MAX),
        (u64::MAX, 0),
    ] {
        assert!(words.part(start..end).is_err(), "{start}..{end}");
    }
    let tail = words.part(bytes - 1..bytes).unwrap();
    assert_eq!(tail.write(&[1; 100]), 1);
    let mut seen = [0; 100];
    assert_eq!(tail.read(&mut seen), 1);
    let end = words.part(bytes..bytes).unwrap();
    assert_eq!(end.write(&[1; 9]), 0);
    assert_eq!(end.read(&mut seen), 0);
}

/// A mapping of no words, and empty parts.
#[test]
fn a_zero_length_region() {
    let words = Words::new(HeapMapping::new(0));
    assert_eq!(words.count(), 0);
    assert!(words.read(0, &mut []).is_ok() && words.write(0, &[]).is_ok());
    assert!(words.read(0, &mut [0]).is_err() && words.write(0, &[0]).is_err());
    assert!(words.read(u64::MAX, &mut []).is_err());
    assert!(words.load(0).is_err() && words.store(0, 0).is_err() && words.swap(0, 0).is_err());
    let part = words.part(0..0).unwrap();
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
    let words = Words::new(HeapMapping::new(2));
    let part = words.part(0..8).unwrap();
    let _ = part.write_from(&mut Liar);
}
