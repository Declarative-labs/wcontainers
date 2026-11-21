//! The Java runtime from an unmodified Alpine image (layer 2): start-up, and a program with
//! threads, garbage collection under load and the safepoints that stop threads through an
//! unreadable page, compared with the reference under several schedules with saves.

use std::path::{Path, PathBuf};

use wcontainers_kernel_linux::reason;
use wcontainers_kernel_test::reference::{GUEST_DIRECTORY, PLATFORM, docker};
use wcontainers_kernel_test::runtimes::{self, JAVA, JDK};

/// Where an exception syndrome's class starts, and the class of a data abort from user space.
const ESR_EC_SHIFT: u64 = 26;
const ESR_EC_DABT_LOW: u64 = 0x24;

/// A directory holding the program's classes, compiled once per test binary.
fn directory() -> PathBuf {
    static DIRECTORY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIRECTORY.get_or_init(compile).clone()
}

fn compile() -> PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("java");
    std::fs::create_dir_all(&out).expect("a directory for the classes");
    let compiled = std::process::Command::new(docker::CLIENT)
        .args([
            docker::START_CONTAINER,
            docker::REMOVE_ON_EXIT,
            docker::PLATFORM_OPTION,
            PLATFORM,
            docker::VOLUME_OPTION,
        ])
        .arg(format!("{}:/src:ro", runtimes::programs("java").display()))
        .arg(docker::VOLUME_OPTION)
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

/// Two saves, at most `gap` turns apart: about a third of the run's turns (measured).
fn saves(gap: u64) -> runtimes::Saves {
    runtimes::Saves { count: 2, gap }
}

#[test]
fn start_up_matches_reference() {
    let runs = runtimes::assert_same(
        JAVA,
        &directory(),
        &["java", "-version"],
        runtimes::Saves {
            count: 1,
            gap: 2_000,
        },
    );
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}

#[test]
fn threads_and_collections_match_reference() {
    let command = [
        "java",
        "-Xmx64m",
        "-XX:+UseG1GC",
        "-cp",
        GUEST_DIRECTORY,
        "Load",
    ];
    let runs = runtimes::assert_same(JAVA, &directory(), &command, saves(8_000));
    eprintln!("warned: {:?}", runtimes::warned(&runs));
    // The compiled loop's polls of the unreadable page faulted and the runtime's handler took
    // them: a data abort's frame was built in every run (without that loop, the program takes
    // no memory faults).
    for run in &runs {
        assert!(
            run.warnings.iter().any(|warning| {
                warning.reason == reason::FAULT_SYNDROME_APPROXIMATED
                    && warning.detail >> ESR_EC_SHIFT == ESR_EC_DABT_LOW
            }),
            "no safepoint poll faulted: {:?}",
            run.warnings
        );
    }
}
