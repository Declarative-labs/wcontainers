use wcontainers_volatile_bytes::StandaloneVolatileBytes;

wcontainers_volatile_bytes_conformance::tests!(wcontainers_volatile_bytes_exclusive_slice::ExclusiveSliceBytes::new(
    &mut [u8::MIN; u16::MAX as usize][..]
));
