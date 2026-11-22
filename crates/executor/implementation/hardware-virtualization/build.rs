//! Tells the linker which Hypervisor.framework calls the build's SDK may not declare, builds the one call Rust
//! cannot make itself, and says whether the host has a hypervisor this crate drives.

use std::env;

use wcontainers_executor_hardware_virtualization_inside::build_environment::{
    AARCH64, CHECK_CFG_END, LATER_HYPERVISOR_CALLS, LEAVE_UNDEFINED, LINK_ARG, MACOS,
    RERUN_IF_ENV_CHANGED, RUSTC_CFG, RUSTC_CHECK_CFG, TARGET_ARCH, TARGET_OS,
};

/// The system KVM is driven on, with the inside's architecture.
const LINUX: &str = "linux";

/// The configuration option set where the host has a hypervisor this crate drives, which `Host` names.
const HOST_HYPERVISOR: &str = "host_hypervisor";

/// The C source of the call that sets a vector register, and the library it is built into.
const VECTOR_SOURCE: &str = "hvf_vector.c";
const VECTOR_LIBRARY: &str = "wcontainers_hvf_vector";

fn main() {
    println!("{RERUN_IF_ENV_CHANGED}{TARGET_OS}");
    println!("{RERUN_IF_ENV_CHANGED}{TARGET_ARCH}");
    println!("{RUSTC_CHECK_CFG}{HOST_HYPERVISOR}{CHECK_CFG_END}");
    let os = env::var(TARGET_OS).unwrap_or_default();
    let arch = env::var(TARGET_ARCH).unwrap_or_default();
    if os == MACOS {
        for call in LATER_HYPERVISOR_CALLS {
            println!("{LINK_ARG}{LEAVE_UNDEFINED}{call}");
        }
        cc::Build::new().file(VECTOR_SOURCE).compile(VECTOR_LIBRARY);
    }
    if os == MACOS || (os == LINUX && arch == AARCH64) {
        println!("{RUSTC_CFG}{HOST_HYPERVISOR}");
    }
}
