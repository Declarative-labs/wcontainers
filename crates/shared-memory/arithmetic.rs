//! The offset arithmetic of `Words`, in the subset of Rust that Verus reads; proof/arithmetic.rs is a copy of it that
//! Verus proves, and proof/agreement.rs tests that the copy and this file agree.

/// The bytes of one word.
pub(crate) const WORD: usize = 8;

/// Where some bytes begin among the words: byte `low` of word `index`, and the `head` bytes they take of that word
/// before the first whole word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) low: usize,
    pub(crate) index: usize,
    pub(crate) head: usize,
}

/// Returns where `length` bytes from the byte offset `at` begin among `count` words, if they all lie inside them.
pub(crate) fn plan(count: usize, at: u64, length: usize) -> Option<Plan> {
    if at.checked_add(length as u64)? > count as u64 * WORD as u64 {
        return None;
    }
    let low = (at % WORD as u64) as usize;
    let room = (WORD - low) % WORD;
    Some(Plan {
        low,
        index: (at / WORD as u64) as usize,
        head: if room < length { room } else { length },
    })
}

/// Returns the start and the length of the bytes at the offsets, if they lie inside `count` words.
pub(crate) fn part(count: usize, start: u64, end: u64) -> Option<(u64, u64)> {
    if start > end || end > count as u64 * WORD as u64 {
        return None;
    }
    Some((start, end - start))
}
