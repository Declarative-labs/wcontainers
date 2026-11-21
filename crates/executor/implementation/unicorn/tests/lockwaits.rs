//! Record lock and flock waits between processes, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn lock_waits_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "lockwaits", &[]);
    // Only the process state files read to see a waiter asleep are counted.
    assert!(
        ours.warnings
            .iter()
            .all(|warned| warned.reason == "proc fields not tracked"),
        "{:?}",
        ours.warnings
    );
}
