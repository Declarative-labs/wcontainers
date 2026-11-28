//! The volatile bytes suite on a view of a host memory that starts and ends inside its pages.

use wcontainers_unicorn::Unicorn;
use wcontainers_unicorn_direct::DirectUnicorn;

/// The page of unicorn's interface.
const PAGE: u64 = 4096;

wcontainers_volatile_bytes_conformance::tests!(
    DirectUnicorn::<()>::host_memory_bytes(
        &DirectUnicorn::<()>::new_host_memory(2 * PAGE).expect("host memory"),
        PAGE / 2..PAGE + PAGE / 2,
    )
    .expect("a view inside the memory")
);
