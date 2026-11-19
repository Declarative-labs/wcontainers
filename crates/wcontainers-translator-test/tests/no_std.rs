//! The kernel uses the cores of the interface and of each CPU crate, which must build without the standard
//! library: for `wasm32v1-none`, which has none, with the `translate` feature off.

use std::path::Path;
use std::process::Command;

#[test]
fn the_cores_build_without_the_standard_library() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("no-std");
    let output = Command::new(env!("CARGO"))
        .current_dir(&root)
        .args([
            "build",
            "-p",
            "wcontainers-guest-cpu",
            "-p",
            "wcontainers-arm64",
            "-p",
            "wcontainers-x86",
        ])
        .args(["--no-default-features", "--target", "wasm32v1-none"])
        .arg("--target-dir")
        .arg(&target)
        .output()
        .expect("cargo runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
