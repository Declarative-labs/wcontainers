//! Names in the environment cargo gives the build scripts that build and embed the inside.

/// The build's output directory.
pub const OUT_DIR: &str = "OUT_DIR";

/// The target's operating system.
pub const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";

/// The target's architecture.
pub const TARGET_ARCH: &str = "CARGO_CFG_TARGET_ARCH";

/// The cargo running the build.
pub const CARGO: &str = "CARGO";
