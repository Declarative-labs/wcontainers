#![no_std]

pub trait RandomSource {
    fn fill_random_bytes(&self, into: &mut [u8]);
}

pub trait StandaloneRandomSource: RandomSource {
    type Config;

    fn new(config: Self::Config) -> Self;
}
