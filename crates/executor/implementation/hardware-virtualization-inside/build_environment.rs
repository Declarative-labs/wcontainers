//! What the build scripts of the inside and the monitor share: names in the environment cargo gives them, and
//! the directives they answer with.

/// The build's output directory.
pub const OUT_DIR: &str = "OUT_DIR";

/// The target's operating system.
pub const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";

/// The target's architecture.
pub const TARGET_ARCH: &str = "CARGO_CFG_TARGET_ARCH";

/// The architecture the inside runs on, by the name cargo gives it.
pub const AARCH64: &str = "aarch64";

/// Runs the build script again when the environment variable named after it changes (and only then, with
/// the script's own code).
pub const RERUN_IF_ENV_CHANGED: &str = "cargo::rerun-if-env-changed=";

/// Runs the build script again when the file or directory after it changes.
pub const RERUN_IF_CHANGED: &str = "cargo::rerun-if-changed=";

/// Gives the linker the option after it, for every target of the package.
pub const LINK_ARG: &str = "cargo::rustc-link-arg=";

/// Gives the linker the option after it, for the package's programs.
pub const LINK_ARG_BINS: &str = "cargo::rustc-link-arg-bins=";

/// Sets the configuration option after it, for every target of the package.
pub const RUSTC_CFG: &str = "cargo::rustc-cfg=";

/// Declares the configuration option between it and [`CHECK_CFG_END`], so the compiler knows the name.
pub const RUSTC_CHECK_CFG: &str = "cargo::rustc-check-cfg=cfg(";
pub const CHECK_CFG_END: &str = ")";
