//! The Java runtime from an unmodified Alpine image (layer 2): start-up, and a program with
//! threads, garbage collection under load and the safepoints that stop threads through an
//! unreadable page, compared with the reference under several schedules with saves.

use std::path::{Path, PathBuf};

use wcontainers_kernel_test::runtimes::{self, JAVA, JDK};

/// A directory holding the program's classes, compiled once per test binary.
fn directory() -> PathBuf {
    static DIRECTORY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIRECTORY.get_or_init(compile).clone()
}

fn compile() -> PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("java");
    std::fs::create_dir_all(&out).expect("a directory for the classes");
    let compiled = std::process::Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "--volume"])
        .arg(format!("{}:/src:ro", runtimes::programs("java").display()))
        .arg("--volume")
        .arg(format!(
            "{}:/out",
            out.canonicalize().expect("the directory").display()
        ))
        .args([JDK, "javac", "-d", "/out", "/src/Load.java"])
        .status()
        .expect("running docker");
    assert!(compiled.success(), "compiling the Java program");
    out
}

#[test]
fn start_up_matches_reference() {
    let runs = runtimes::assert_same(JAVA, &directory(), &["java", "-version"], 1);
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}

#[test]
fn threads_and_collections_match_reference() {
    let command = ["java", "-Xmx64m", "-XX:+UseG1GC", "-cp", "/wc", "Load"];
    let runs = runtimes::assert_same(JAVA, &directory(), &command, 2);
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}
