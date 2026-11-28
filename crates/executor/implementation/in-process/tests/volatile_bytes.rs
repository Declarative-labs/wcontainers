//! The volatile bytes suite on the bytes a form I memory lends, across the boundary between two pages.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use wcontainers_executor::Executor;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor_in_process::execution::InProcess;
use wcontainers_executor_in_process::memory::{GUEST_PAGE, InProcessPool};

/// The addresses lent: the second half of one page and the first half of the next.
const ADDRESSES: core::ops::Range<u64> = PAGE + GUEST_PAGE / 2..PAGE + GUEST_PAGE + GUEST_PAGE / 2;
/// A page of the window.
const PAGE: u64 = 0x10_0000;

wcontainers_volatile_bytes_conformance::tests!(
    InProcess::new(InProcessPool::new().expect("a host with the guest's pages"))
        .create_guest_memory()
        .expect("a free slot")
        .0
        .borrow(ADDRESSES)
        .expect("addresses inside the window")
);
