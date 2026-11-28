use alloc::collections::VecDeque;
use core::convert::Infallible;

use vstd::prelude::*;

use crate::ByteSource;

verus! {

/// Takes the bytes read from the front of the queue.
#[allow(reviewable::implementation_not_verified, reason = "TODO: prove it against the trait's contract, then remove this line")]
#[verifier::external]
impl ByteSource for VecDeque<u8> {
    type Error = Infallible;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Infallible> {
        let count = into.len().min(self.len());
        let (front, back) = self.as_slices();
        let from_front = count.min(front.len());
        into[..from_front].copy_from_slice(&front[..from_front]);
        into[from_front..count].copy_from_slice(&back[..count - from_front]);
        self.drain(..count);
        Ok(count)
    }
}

}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn the_bytes_read_leave_the_front_of_the_queue() {
        let bytes = b"queued bytes";
        let mut queue = VecDeque::from(*bytes);
        let mut buffer = [u8::MIN; size_of::<u64>()];
        assert_eq!(queue.read(&mut buffer), Ok(buffer.len()));
        assert_eq!(buffer, bytes[..buffer.len()]);
        assert_eq!(queue, &bytes[buffer.len()..]);
    }

    #[test]
    fn a_queue_that_wraps_around_is_read_whole_in_order() {
        let bytes = b"queued bytes";
        let moved = bytes.len() / size_of::<u16>();
        let mut queue = VecDeque::with_capacity(bytes.len());
        queue.extend(bytes);
        queue.drain(..moved);
        queue.extend(&bytes[..moved]);
        let held: Vec<u8> = queue.iter().copied().collect();
        let mut buffer = [u8::MIN; b"queued bytes".len()];
        assert_eq!(queue.read(&mut buffer), Ok(bytes.len()));
        assert_eq!(buffer, held.as_slice());
        assert!(queue.is_empty());
    }
}
