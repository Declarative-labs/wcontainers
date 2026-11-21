//! Opens with a trailing slash and at the descriptor limit, copies between file systems, device
//! positions and `/proc`'s own attributes, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_linux::reason;
use wcontainers_kernel_test::assert_same_output;

#[test]
fn opens_and_copies_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "opencheck", &[]);
    // `O_TMPFILE` is not built; it is refused only after the descriptor is found.
    assert!(
        ours.counters
            .iter()
            .all(|counter| counter.reason != reason::NOT_IMPLEMENTED)
    );
}
