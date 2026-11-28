//! The volatile bytes suite on the bytes a space of the stand-in lends, across the boundary between two pages.

use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE};
use wcontainers_executor::Executor;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor_unicorn::standin::StandIn;

/// The addresses lent: the second half of one page and the first half of the next.
const ADDRESSES: std::ops::Range<u64> =
    LOWEST_ADDRESS + PAGE_SIZE / 2..LOWEST_ADDRESS + PAGE_SIZE + PAGE_SIZE / 2;

wcontainers_volatile_bytes_conformance::tests!(
    StandIn::new()
        .create_guest_memory()
        .expect("a space")
        .0
        .borrow(ADDRESSES)
        .expect("addresses inside the space")
);
