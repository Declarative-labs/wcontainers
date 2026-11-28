//! Builds, for the earlier-action test on Linux on ARM64, the library a child preloads: a handler of one argument.

use std::env;
use std::path::PathBuf;

/// The variables naming the target's operating system and architecture, and the build's output folder.
const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";
const TARGET_ARCH: &str = "CARGO_CFG_TARGET_ARCH";
const OUT_DIR: &str = "OUT_DIR";

/// The target the test runs on.
const LINUX: &str = "linux";
const AARCH64: &str = "aarch64";

/// The instructions to cargo that rerun the script when a file changes and set a variable for the test.
const RERUN_IF_CHANGED: &str = "cargo::rerun-if-changed=";
const RUSTC_ENV: &str = "cargo::rustc-env=";

/// The C source of the handler, the library it is built into, and the variable that names the library.
const HANDLER_SOURCE: &str = "one_argument_handler.c";
const HANDLER_LIBRARY: &str = "libone_argument_handler.so";
const HANDLER_VARIABLE: &str = "ONE_ARGUMENT_HANDLER";

fn main() {
    println!("{RERUN_IF_CHANGED}{HANDLER_SOURCE}");
    let target = (env::var(TARGET_OS), env::var(TARGET_ARCH));
    if !matches!(target, (Ok(os), Ok(arch)) if os == LINUX && arch == AARCH64) {
        return;
    }
    let library = PathBuf::from(env::var(OUT_DIR).expect("cargo sets the output folder"))
        .join(HANDLER_LIBRARY);
    let built = cc::Build::new()
        .get_compiler()
        .to_command()
        .args(["-shared", "-fPIC", "-o"])
        .arg(&library)
        .arg(HANDLER_SOURCE)
        .status()
        .expect("the C compiler runs");
    assert!(built.success(), "the handler library builds");
    println!("{RUSTC_ENV}{HANDLER_VARIABLE}={}", library.display());
}
