//! The volatile bytes suite on the memory given to the VM: anonymous pages, mapped shared.
#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use wcontainers_hypervisor_framework::HypervisorFramework;
use wcontainers_hypervisor_framework_direct::DirectHypervisorFramework;

/// The bytes of the memory: two pages of the largest size a host uses.
const LENGTH: usize = 2 << 16;

wcontainers_volatile_bytes_conformance::tests!(
    DirectHypervisorFramework
        .create_user_memory(LENGTH)
        .expect("anonymous memory mapped")
);
