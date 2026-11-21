//! A Go program (layer 2), built by Go's own image and run on the reference image: the runtime's
//! scheduler with many goroutines on four threads, its network poller on loopback, its signal
//! handling and its use of futex and clone, compared with the reference under several schedules
//! with saves.

use std::path::{Path, PathBuf};

use wcontainers_executor_unicorn::reference::{IMAGE, PLATFORM, docker};
use wcontainers_executor_unicorn::runtimes::{self, GO};

/// A directory holding the program, built once per test binary.
fn directory() -> PathBuf {
    static DIRECTORY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIRECTORY.get_or_init(build).clone()
}

fn build() -> PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("go");
    std::fs::create_dir_all(&out).expect("a directory for the program");
    let built = std::process::Command::new(docker::CLIENT)
        .args([
            docker::START_CONTAINER,
            docker::REMOVE_ON_EXIT,
            docker::PLATFORM_OPTION,
            PLATFORM,
            docker::VOLUME_OPTION,
        ])
        .arg(format!("{}:/src:ro", runtimes::programs("go").display()))
        .arg(docker::VOLUME_OPTION)
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

/// Two saves, at most `gap` turns apart: about a third of the run's turns (measured).
fn saves(gap: u64) -> runtimes::Saves {
    runtimes::Saves { count: 2, gap }
}

#[test]
fn program_matches_reference() {
    let runs = runtimes::assert_same(IMAGE, &directory(), &["/wc/program"], saves(2_000));
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}
