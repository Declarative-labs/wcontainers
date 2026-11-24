use alloc::collections::TryReserveError;
use alloc::vec::Vec;

use crate::ByteSink;

/// Appends all the bytes to the vector.
impl ByteSink for Vec<u8> {
    /// The vector could not grow by the bytes.
    type Error = TryReserveError;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, TryReserveError> {
        self.try_reserve(bytes.len())?;
        self.extend_from_slice(bytes);
        Ok(bytes.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vector_grows_by_the_bytes_after_what_it_held() {
        let bytes = b"appended bytes";
        let mut vector = bytes.to_vec();
        assert_eq!(vector.write(bytes), Ok(bytes.len()));
        assert_eq!(vector, [bytes.as_slice(), bytes].concat());
    }
}
