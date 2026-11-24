use core::convert::Infallible;
use core::mem;

use crate::ByteSink;

/// Writes to the start of the slice, which then starts after the bytes written.
impl ByteSink for &mut [u8] {
    type Error = Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, Infallible> {
        let count = bytes.len().min(self.len());
        let (written, rest) = mem::take(self).split_at_mut(count);
        written.copy_from_slice(&bytes[..count]);
        *self = rest;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_is_written_from_its_start_and_then_starts_after_the_bytes_written() {
        let bytes = b"written";
        let mut room = [u8::MIN; b"bytes to write".len()];
        let mut slice: &mut [u8] = &mut room;
        assert_eq!(slice.write(bytes), Ok(bytes.len()));
        assert_eq!(slice.len(), b"bytes to write".len() - bytes.len());
        assert_eq!(room[..bytes.len()], *bytes);
        assert!(room[bytes.len()..].iter().all(|&byte| byte == u8::MIN));
    }

    #[test]
    fn a_slice_shorter_than_the_bytes_takes_as_many_as_it_holds() {
        let bytes = b"bytes to write";
        let mut room = [u8::MIN; size_of::<u64>()];
        let mut slice: &mut [u8] = &mut room;
        assert_eq!(slice.write(bytes), Ok(size_of::<u64>()));
        assert!(slice.is_empty());
        assert_eq!(room, bytes[..size_of::<u64>()]);
    }
}
