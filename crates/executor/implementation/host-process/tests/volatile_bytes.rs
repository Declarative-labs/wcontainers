//! The volatile bytes suite on the bytes a form P memory lends, across the boundary between two pages.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::{Arc, OnceLock};

use wcontainers_executor::Executor;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor_host_process::executor::HostProcess;
use wcontainers_executor_host_process::host::HostProcessHost;

/// The guest's page size, and the addresses lent: the second half of one page and the first half of the next.
const PAGE_BYTES: u64 = 1 << 12;
const ADDRESSES: core::ops::Range<u64> = PAGE + PAGE_BYTES / 2..PAGE + PAGE_BYTES + PAGE_BYTES / 2;
/// A page of the guest's.
const PAGE: u64 = 0x10_0000;

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

wcontainers_volatile_bytes_conformance::tests!(
    HostProcess::new(Arc::clone(
        HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
    ))
    .create_guest_memory()
    .expect("a memory")
    .0
    .borrow(ADDRESSES)
    .expect("addresses inside the space")
);
