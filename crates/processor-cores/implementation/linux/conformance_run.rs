#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

wcontainers_processor_cores_conformance::tests!(
    wcontainers_processor_cores_linux::LinuxProcessorCores
);
