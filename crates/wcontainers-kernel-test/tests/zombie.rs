//! Calls that name a child that has ended and is not yet reaped, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::assert_same_output;

#[test]
fn ended_children_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "zombie", &[]);
    // Only the counted approximations of `stat`, `statm` and `status`.
    assert!(
        ours.warnings
            .iter()
            .all(|warning| warning.reason == reason::PROC_FIELDS_NOT_TRACKED),
        "{:?}",
        ours.warnings
    );
}
