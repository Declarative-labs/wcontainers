#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

wcontainers_kvm_conformance::tests!(wcontainers_kvm_direct::DirectKvm);
