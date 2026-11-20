//! The generator's key is laid out as the harness wrote it by hand before, so recorded seeds replay.

use proptest::prelude::RngExt;
use proptest::test_runner::{RngAlgorithm, TestRng};
use wcontainers_translator_test::seeded::generator;

fn first_words(mut rng: TestRng) -> [u64; 4] {
    std::array::from_fn(|_| rng.random::<u64>())
}

#[test]
fn keys_replay_the_hand_made_ones() {
    let seed = 0x0123_4567_89ab_cdef;
    let name = "MSR_SR_systemmove_with_a_long_name";
    let by_hand = |tag: &[u8], folded_from: usize| {
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&u64::to_le_bytes(seed));
        key[8..8 + tag.len()].copy_from_slice(tag);
        if folded_from < 32 {
            for (i, byte) in name.bytes().enumerate() {
                key[folded_from + i % (32 - folded_from)] ^= byte;
            }
        }
        TestRng::from_seed(RngAlgorithm::ChaCha, &key)
    };
    assert_eq!(
        first_words(generator(seed, &[], "")),
        first_words(by_hand(&[], 32))
    );
    assert_eq!(
        first_words(generator(seed, b"some tag", "")),
        first_words(by_hand(b"some tag", 32))
    );
    assert_eq!(
        first_words(generator(seed, &[], name)),
        first_words(by_hand(&[], 8))
    );
    assert_eq!(
        first_words(generator(seed, b"all form", name)),
        first_words(by_hand(b"all form", 16))
    );
}
