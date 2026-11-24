use alloc::collections::{TryReserveError, VecDeque};

use crate::ByteSink;

/// Appends all the bytes to the back of the queue.
impl ByteSink for VecDeque<u8> {
    /// The queue could not grow by the bytes.
    type Error = TryReserveError;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, TryReserveError> {
        self.try_reserve(bytes.len())?;
        self.extend(bytes);
        Ok(bytes.len())
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn a_queue_grows_at_its_back_by_the_bytes_where_they_wrap_around_its_buffer() {
        let bytes = b"bytes pushed to the back";
        let moved = bytes.len() / size_of::<u16>();
        let mut queue = VecDeque::with_capacity(bytes.len());
        let capacity = queue.capacity();
        queue.resize(capacity, u8::MIN);
        queue.drain(..moved);
        let held: Vec<u8> = queue.iter().copied().collect();
        assert_eq!(queue.write(&bytes[..moved]), Ok(moved));
        assert_eq!(queue.capacity(), capacity, "the write fit in the room the drain left at the front");
        assert!(!queue.as_slices().1.is_empty(), "the queue wraps around its buffer");
        assert_eq!(queue, [held.as_slice(), &bytes[..moved]].concat());
    }
}
