//! The offset arithmetic of the region, apart from its unsafe code so that Kani checks it for every method.

use crate::{OutsideError, WordError};

/// Returns the offset as an index into `length` bytes, if the `count` bytes from it all lie inside them.
pub(crate) fn bytes(length: usize, at: u64, count: usize) -> Result<usize, OutsideError> {
    let end = at
        .checked_add(u64::try_from(count).map_err(|_| OutsideError)?)
        .ok_or(OutsideError)?;
    if end > u64::try_from(length).map_err(|_| OutsideError)? {
        return Err(OutsideError);
    }
    usize::try_from(at).map_err(|_| OutsideError)
}

/// Returns the offset as an index into `length` bytes of a word of `size` bytes, if the word lies inside them and the
/// offset is a multiple of `size`.
pub(crate) fn word(length: usize, at: u64, size: usize) -> Result<usize, WordError> {
    let index = bytes(length, at, size)?;
    if !index.is_multiple_of(size) {
        return Err(WordError::Misaligned);
    }
    Ok(index)
}
