//! A Go program (layer 2), built by Go's own image and run on the reference image: the runtime's
//! scheduler with many goroutines on four threads, its network poller on loopback, its signal
//! handling and its use of futex and clone, compared with the reference under several schedules
//! with saves.

use std::path::{Path, PathBuf};

use wcontainers_kernel_test::reference::IMAGE;
use wcontainers_kernel_test::runtimes::{self, GO};

/// A directory holding the program, built once per test binary.
fn directory() -> PathBuf {
    static DIRECTORY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIRECTORY.get_or_init(build).clone()
}

fn build() -> PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("go");
    std::fs::create_dir_all(&out).expect("a directory for the program");
    let built = std::process::Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "--volume"])
        .arg(format!("{}:/src:ro", runtimes::programs("go").display()))
        .arg("--volume")
        .arg(format!(
            "{}:/out",
            out.canonicalize().expect("the directory").display()
        ))
        .args(["--workdir", "/src", "--env", "CGO_ENABLED=0", GO])
        .args(["go", "build", "-trimpath", "-o", "/out/program", "main.go"])
        .status()
        .expect("running docker");
    assert!(built.success(), "building the Go program");
    out
}

#[test]
fn program_matches_reference() {
    let runs = runtimes::assert_same(IMAGE, &directory(), &["/wc/program"], 2);
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}
