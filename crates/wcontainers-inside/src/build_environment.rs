//! What the build scripts of the inside and the monitor share: names in the environment cargo gives them, and
//! the directives they answer with.

/// The build's output directory.
pub const OUT_DIR: &str = "OUT_DIR";

/// The target's operating system.
pub const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";

/// Runs the build script again when the environment variable named after it changes (and only then, with
/// the script's own code).
pub const RERUN_IF_ENV_CHANGED: &str = "cargo::rerun-if-env-changed=";

/// Gives the linker the option after it, for every target of the package.
pub const LINK_ARG: &str = "cargo::rustc-link-arg=";

/// Gives the linker the option after it, for the package's programs.
pub const LINK_ARG_BINS: &str = "cargo::rustc-link-arg-bins=";
