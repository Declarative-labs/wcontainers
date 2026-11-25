//! Builds, on macOS, the one call Rust cannot make itself: setting a vector register.

use std::env;

/// The variable naming the target's operating system.
const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";

/// The operating system Hypervisor.framework runs on.
const MACOS: &str = "macos";

/// The instruction to cargo that reruns the script when a file changes.
const RERUN_IF_CHANGED: &str = "cargo::rerun-if-changed=";

/// The C source of the call that sets a vector register, and the library it is built into.
const VECTOR_SOURCE: &str = "vector.c";
const VECTOR_LIBRARY: &str = "wcontainers_hypervisor_framework_vector";

fn main() {
    println!("{RERUN_IF_CHANGED}{VECTOR_SOURCE}");
    if env::var(TARGET_OS).is_ok_and(|os| os == MACOS) {
        cc::Build::new().file(VECTOR_SOURCE).compile(VECTOR_LIBRARY);
    }
}
