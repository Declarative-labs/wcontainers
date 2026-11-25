//! The harness's random generators, each made from a seed so that every random case replays.

use proptest::test_runner::{RngAlgorithm, TestRng};

/// The length of the key proptest's ChaCha generator takes.
pub const CHACHA_KEY_BYTES: usize = 32;

/// A ChaCha generator keyed by `seed`, then `tag`, then `name` folded over the rest of the key, so that each
/// use of one seed draws a sequence of its own.
pub fn generator(seed: u64, tag: &[u8], name: &str) -> TestRng {
    let mut key = [0u8; CHACHA_KEY_BYTES];
    let seed = seed.to_le_bytes();
    let (start, rest) = key.split_at_mut(seed.len());
    start.copy_from_slice(&seed);
    let (tagged, rest) = rest.split_at_mut(tag.len());
    tagged.copy_from_slice(tag);
    let folded = rest.len();
    for (i, byte) in name.bytes().enumerate() {
        rest[i % folded] ^= byte;
    }
    TestRng::from_seed(RngAlgorithm::ChaCha, &key)
}
