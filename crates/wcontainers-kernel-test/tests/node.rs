//! Node from an unmodified Alpine image (layer 2): start-up, and a program with timers, child
//! processes, worker threads, file streams and an HTTP server and client on loopback, each
//! compared with the reference under several schedules with saves.

use wcontainers_kernel_test::runtimes::{self, NODE};

/// Two saves, at most `gap` turns apart: about a third of the run's turns (measured).
fn saves(gap: u64) -> runtimes::Saves {
    runtimes::Saves { count: 2, gap }
}

#[test]
fn start_up_matches_reference() {
    let directory = runtimes::programs("node");
    let runs = runtimes::assert_same(
        NODE,
        &directory,
        &[
            "node",
            "-e",
            "console.log(process.version, [1, 2, 3].map((x) => x * 2))",
        ],
        runtimes::Saves { count: 1, gap: 700 },
    );
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}

#[test]
fn system_program_matches_reference() {
    let directory = runtimes::programs("node");
    let runs = runtimes::assert_same(NODE, &directory, &["node", "/wc/system.js"], saves(7_000));
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}
