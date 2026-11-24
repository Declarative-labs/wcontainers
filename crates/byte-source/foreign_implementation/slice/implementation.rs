use core::convert::Infallible;

use crate::ByteSource;

/// Reads from the start of the slice, which then starts after the bytes read.
impl ByteSource for &[u8] {
    type Error = Infallible;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Infallible> {
        let (read, rest) = self.split_at(into.len().min(self.len()));
        into[..read.len()].copy_from_slice(read);
        *self = rest;
        Ok(read.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_is_read_from_its_start_and_then_starts_after_the_bytes_read() {
        let bytes = b"a slice to read";
        let mut slice: &[u8] = bytes;
        let mut buffer = [u8::MIN; size_of::<u64>()];
        assert_eq!(slice.read(&mut buffer), Ok(buffer.len()));
        assert_eq!(buffer, bytes[..buffer.len()]);
        assert_eq!(slice, &bytes[buffer.len()..]);
    }

    #[test]
    fn a_slice_shorter_than_the_buffer_is_read_whole() {
        let bytes = b"short";
        let mut slice: &[u8] = bytes;
        let mut buffer = [u8::MIN; size_of::<u64>()];
        assert_eq!(slice.read(&mut buffer), Ok(bytes.len()));
        assert_eq!(buffer[..bytes.len()], *bytes);
        assert!(buffer[bytes.len()..].iter().all(|&byte| byte == u8::MIN));
        assert!(slice.is_empty());
    }
}
