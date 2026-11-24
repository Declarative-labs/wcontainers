//! Volatile accesses only, for any other architecture and for Miri and Kani, which run no assembly; a swap or a
//! compare-and-exchange here is a load and then a store, not one atomic access.

pub(super) use self::{
    compare_exchange as compare_exchange_u32, compare_exchange as compare_exchange_u64,
    swap as swap_u32, swap as swap_u64,
};

/// Copies `length` bytes from `from` to `to`, one volatile load and one volatile store of each byte.
///
/// # Safety
/// `from` is readable and `to` writable for `length` bytes, and the two ranges do not overlap.
#[expect(unsafe_code, reason = "the copy, in both directions")]
pub(super) unsafe fn copy(to: *mut u8, from: *const u8, length: usize) {
    for index in 0..length {
        // SAFETY: the byte lies inside both ranges, which the caller promises.
        unsafe {
            to.wrapping_add(index)
                .write_volatile(from.wrapping_add(index).read_volatile());
        }
    }
}

/// Stores the value in the word and returns the value it held.
///
/// # Safety
/// The word at `word` is writable and aligned.
#[expect(unsafe_code, reason = "a swap of a word")]
pub(super) unsafe fn swap<W: Copy>(word: *mut W, value: W) -> W {
    // SAFETY: the word is the caller's to read and write.
    unsafe {
        let old = word.read_volatile();
        word.write_volatile(value);
        old
    }
}

/// Stores `new` in the word if it holds `current`, and returns the value it held.
///
/// # Safety
/// The word at `word` is writable and aligned.
#[expect(unsafe_code, reason = "a compare-and-exchange of a word")]
pub(super) unsafe fn compare_exchange<W: Copy + Eq>(word: *mut W, current: W, new: W) -> W {
    // SAFETY: the word is the caller's to read and write.
    unsafe {
        let old = word.read_volatile();
        if old == current {
            word.write_volatile(new);
        }
        old
    }
}
