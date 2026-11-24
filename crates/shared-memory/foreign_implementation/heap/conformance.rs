//! The heap mapping exists only under Miri and Kani, so this suite runs only under Miri.
#![cfg(miri)]

use wcontainers_shared_memory::{HeapMapping, SharedMemory};

/// The words of each mapping: those the suite needs, and three more.
const WORDS: usize = wcontainers_shared_memory_conformance::NEEDED as usize / size_of::<u64>() + 3;

/// A page as large as the whole mapping, which is all a page boundary test can reach on it.
const PAGE: usize = WORDS * size_of::<u64>();

wcontainers_shared_memory_conformance::tests!(HeapMapping::new(WORDS), PAGE);

mod volatile_bytes {
    use super::*;

    wcontainers_volatile_bytes_conformance::tests!(
        HeapMapping::new(WORDS)
            .borrow(3..PAGE as u64 - 1)
            .expect("the offsets lie inside the mapping")
    );
}

#[test]
fn an_empty_mapping_refuses_every_word_and_byte() {
    let memory = HeapMapping::new(0);
    assert_eq!(memory.len(), 0);
    assert!(memory.load::<u64>(0).is_err() && memory.store(0, 0_u32).is_err());
    assert!(memory.swap(0, 0_u64).is_err() && memory.compare_exchange(0, 0_u32, 1).is_err());
    assert!(memory.read(0, &mut [0]).is_err() && memory.write(0, &[0]).is_err());
    assert!(memory.read(0, &mut []).is_ok() && memory.write(0, &[]).is_ok());
    assert!(memory.borrow(0..0).is_ok() && memory.borrow(0..1).is_err());
}
