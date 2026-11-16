//! A running container saved at moments drawn from a seed and brought back each time on a new CPU
//! side finishes as the reference does. `SAVING_SEEDS` sets how many seeds run (default 3).

use std::path::Path;

use wcontainers_kernel_test::{guest, reference, run_ours_saving};

#[test]
fn saved_and_brought_back_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("processes", &[], &out);
    let theirs = reference::run(&program, &[]);
    let seeds: u64 = std::env::var("SAVING_SEEDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    for seed in 1..=seeds {
        let ours = run_ours_saving(
            program.parent().expect("a directory"),
            "/wc/processes",
            &["/wc/processes"],
            seed,
            8,
        );
        assert_eq!(
            String::from_utf8_lossy(&ours.outcome.stdout),
            String::from_utf8_lossy(&theirs.stdout),
            "seed {seed}"
        );
        assert_eq!(ours.outcome, theirs, "seed {seed}");
        assert_eq!(ours.saved, 8, "seed {seed}");
    }
}
