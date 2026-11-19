//! musl's own test suite (libc-test), each program run by the suite's runner as a container's first
//! process on our kernel and in the reference, which must end the same way and print the same (the
//! suite prints only what failed). The tests of threads, locks, signals and timers run under every seed of
//! `SCHEDULE_SEEDS`; `LIBC_TEST` picks tests by a part of their names and `LIBC_TEST_JOBS` how many
//! run side by side.

use std::path::Path;

use wcontainers_kernel_test::libctest::{self, Test};
use wcontainers_kernel_test::{reference, run_ours_saving, schedule_seeds};

/// Tests whose outcome here differs from the reference, each with the gap it comes from.
const KNOWN: &[(&str, &str)] = &[(
    "functional-socket",
    "sockets are not built yet (K5): socket() is ENOSYS",
)];

/// Whether a test's program takes turns between threads or waits on timers and signals, so that
/// where turns end matters.
fn scheduled(name: &str) -> bool {
    [
        "pthread",
        "sem_",
        "tls_",
        "raise",
        "sig",
        "clock",
        "time",
        "vfork",
        "spawn",
        "popen",
        "daemon",
        "ipc_",
        "flockfile",
    ]
    .iter()
    .any(|part| name.contains(part))
}

fn chosen(name: &str) -> bool {
    std::env::var("LIBC_TEST").map_or(true, |part| name.contains(&part))
}

/// Runs the tests that `wanted` accepts under `seeds`, built into a directory of their own under
/// `label`, and fails with every difference not known.
fn compare(label: &str, wanted: impl Fn(&str) -> bool, seeds: std::ops::Range<u64>, saves: usize) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("libc-test")
        .join(label);
    let tests = libctest::build(&out, |name| wanted(name) && chosen(name));
    let theirs = libctest::each(libctest::jobs(), &tests, |test| {
        let arguments = test.arguments();
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        reference::run(&test.runner, &arguments)
    });
    let runs: Vec<(&Test, u64)> = tests
        .iter()
        .flat_map(|test| seeds.clone().map(move |seed| (test, seed)))
        .collect();
    let ours = libctest::each(libctest::jobs(), &runs, |(test, seed)| {
        let directory = test.runner.parent().expect("a directory");
        let arguments = test.arguments();
        let mut all = vec!["/wc/runtest"];
        all.extend(arguments.iter().map(String::as_str));
        let started = std::time::Instant::now();
        let run = run_ours_saving(directory, "/wc/runtest", &all, *seed, saves);
        eprintln!("{} seed {seed}: {:?}", test.name, started.elapsed());
        run
    });
    let mut differences = Vec::new();
    let mut known_passing = Vec::new();
    for ((test, seed), ours) in runs.iter().zip(&ours) {
        let index = tests
            .iter()
            .position(|other| other.name == test.name)
            .expect("a test");
        let (ours, theirs) = (
            libctest::verdict(&ours.outcome),
            libctest::verdict(&theirs[index]),
        );
        let known = KNOWN.iter().find(|(name, _)| *name == test.name);
        match (ours == theirs, known) {
            (true, Some(_)) => known_passing.push(format!("{} seed {seed}", test.name)),
            (false, None) => differences.push(format!(
                "{} seed {seed}:\n  ours   {ours}\n  theirs {theirs}",
                test.name
            )),
            _ => {}
        }
    }
    println!(
        "{} tests, {} runs on ours; known failures that matched: {:?}",
        tests.len(),
        runs.len(),
        known_passing
    );
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn threads_signals_and_timers_match_reference() {
    compare("scheduled", scheduled, schedule_seeds(), 0);
}

/// The same with saves at moments drawn from each seed.
#[test]
fn threads_signals_and_timers_survive_saving() {
    compare("saved", scheduled, schedule_seeds(), 4);
}

#[test]
fn the_rest_matches_reference() {
    compare("rest", |name| !scheduled(name), 1..2, 0);
}
