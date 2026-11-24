//! The heap mapping exists only under Miri and Kani, so this suite runs only under Miri.
#![cfg(miri)]

use wcontainers_shared_memory::{HeapMapping, Words};

/// The words of each mapping: those the suite needs, and three more.
const WORDS: usize = wcontainers_shared_memory_conformance::NEEDED as usize + 3;

wcontainers_shared_memory_conformance::tests!(Words::new(HeapMapping::new(WORDS)));

mod volatile_bytes {
    use super::*;

    wcontainers_volatile_bytes_conformance::tests!(
        Words::new(HeapMapping::new(WORDS))
            .part(3..WORDS as u64 * 8 - 1)
            .expect("the offsets lie inside the mapping")
    );
}

#[test]
fn an_empty_mapping_refuses_every_word_and_byte() {
    let words = Words::new(HeapMapping::new(0));
    assert_eq!(words.count(), 0);
    assert!(words.load(0).is_err() && words.store(0, 0).is_err() && words.swap(0, 0).is_err());
    assert!(words.read(0, &mut [0]).is_err() && words.write(0, &[0]).is_err());
    assert!(words.read(0, &mut []).is_ok() && words.write(0, &[]).is_ok());
    assert!(words.part(0..0).is_ok() && words.part(0..1).is_err());
}
