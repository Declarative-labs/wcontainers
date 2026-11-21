#![no_std]

pub trait RandomSource {
    fn fill_random_bytes(&self, into: &mut [u8]);
}
