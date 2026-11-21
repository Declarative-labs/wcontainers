//! Tells the linker which Hypervisor.framework calls the build's SDK may not declare, builds the one call Rust
//! cannot make itself, and says whether the host has a hypervisor this crate drives.

use std::env;

use wcontainers_executor_hardware_virtualization_inside::build_environment::{
    AARCH64, CHECK_CFG_END, LINK_ARG, RERUN_IF_ENV_CHANGED, RUSTC_CFG, RUSTC_CHECK_CFG,
    TARGET_ARCH, TARGET_OS,
};

/// The system whose SDK may lack [`LATER_CALLS`].
const MACOS: &str = "macos";

/// The system KVM is driven on, with the inside's architecture.
const LINUX: &str = "linux";

/// The configuration option set where the host has a hypervisor this crate drives, which `Host` names.
const HOST_HYPERVISOR: &str = "host_hypervisor";

/// The linker option that leaves a symbol, named after it, to be found when the program loads.
const LEAVE_UNDEFINED: &str = "-Wl,-U,_";

/// Hypervisor.framework calls of macOS 15 and later that the build's SDK may not declare: the linker leaves
/// them to be found when the program loads, on a system that has them.
const LATER_CALLS: &[&str] = &[
    "hv_gic_config_create",
    "hv_gic_config_set_distributor_base",
    "hv_gic_config_set_redistributor_base",
    "hv_gic_create",
    "hv_gic_get_intid",
    "hv_gic_get_spi_interrupt_range",
    "hv_gic_set_spi",
];

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
        for call in LATER_CALLS {
            println!("{LINK_ARG}{LEAVE_UNDEFINED}{call}");
        }
        cc::Build::new().file(VECTOR_SOURCE).compile(VECTOR_LIBRARY);
    }
    if os == MACOS || (os == LINUX && arch == AARCH64) {
        println!("{RUSTC_CFG}{HOST_HYPERVISOR}");
    }
}
