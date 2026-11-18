//! inotify compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn inotify_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "inotify", &[]);
}

#[test]
#[ignore = "known difference: on a backend root, events through an open file whose name was \
            unlinked while another link names it reach IN_EXCL_UNLINK watches (guest/inotify.c's \
            writes after unlink); see the status file"]
fn inotify_matches_reference_on_backend_root() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_kernel_test::assert_same_output_on_backend_root_seeded(&out, "inotify", &[], 0);
}
