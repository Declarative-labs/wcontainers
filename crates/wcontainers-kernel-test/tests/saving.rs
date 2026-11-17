//! A running container saved at moments drawn from a seed and brought back each time from the copy
//! alone (a new process holding a copy of the kernel's memory, a new CPU side made from the saved
//! memories and registers, and a new host whose monotonic clock reads from an earlier start)
//! finishes as the reference does. `SAVING_SEEDS` sets how many seeds run for each program (default
//! 3).

use std::path::Path;

use wcontainers_kernel_test::{guest, reference, run_ours_saving};

/// How many times each run is saved and brought back.
const SAVES: usize = 8;

fn saved_and_brought_back(name: &str) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build(name, &[], &out);
    let theirs = reference::run(&program, &[]);
    let seeds: u64 = std::env::var("SAVING_SEEDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    let path = format!("/wc/{name}");
    for seed in 1..=seeds {
        let ours = run_ours_saving(
            program.parent().expect("a directory"),
            &path,
            &[&path],
            seed,
            SAVES,
        );
        assert_eq!(
            String::from_utf8_lossy(&ours.outcome.stdout),
            String::from_utf8_lossy(&theirs.stdout),
            "{name} seed {seed}"
        );
        assert_eq!(ours.outcome, theirs, "{name} seed {seed}");
        assert_eq!(ours.saved, SAVES, "{name} seed {seed}");
    }
}

/// One test, so that no other test's thread is running when a save copies the process.
#[test]
fn saved_and_brought_back_match_reference() {
    for name in ["processes", "deadlines", "clockwatch", "unixsock", "epoll"] {
        saved_and_brought_back(name);
    }
}
