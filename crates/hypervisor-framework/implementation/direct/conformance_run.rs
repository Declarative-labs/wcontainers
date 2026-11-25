#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

wcontainers_hypervisor_framework_conformance::tests!(
    wcontainers_hypervisor_framework_direct::DirectHypervisorFramework
);
