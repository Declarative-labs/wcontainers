//! Signal frames, fault information and handler returns, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::assert_same_output;

#[test]
fn signal_frames_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, "sigframe", &[]);
    // Only a translation fault's level is approximated in a frame.
    assert!(
        ours.warnings
            .iter()
            .all(|warned| warned.reason == reason::FAULT_SYNDROME_APPROXIMATED),
        "{:?}",
        ours.warnings
    );
}
