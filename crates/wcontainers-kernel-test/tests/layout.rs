//! Which parts of a started program are placed at random, and the settings that say so, compared
//! with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn random_layout_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let run = assert_same_output(&out, "layout", &[]);
    print!("{}", String::from_utf8_lossy(&run.outcome.stdout));
}
