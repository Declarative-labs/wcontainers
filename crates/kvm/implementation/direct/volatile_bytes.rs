//! The volatile bytes suite on the memory a slot is given: a sealed memory file's pages, mapped shared.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use wcontainers_kvm::Kvm;
use wcontainers_kvm_direct::DirectKvm;

/// The bytes of the memory: two pages of the largest size a host uses.
const LENGTH: usize = 2 << 16;

wcontainers_volatile_bytes_conformance::tests!(
    DirectKvm
        .create_user_memory(LENGTH)
        .expect("a memory file mapped")
);
