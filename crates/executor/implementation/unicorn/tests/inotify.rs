//! inotify compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn inotify_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "inotify", &[]);
}

#[test]
#[ignore = "known difference: on a backend root, events through an open file renamed since it \
            was opened name its old name, not its new one (guest/inotify.c's writes to moved c); \
            see the status file"]
fn inotify_matches_reference_on_backend_root() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output_on_backend_root_seeded(
        &out,
        "inotify",
        &[],
        0,
    );
}

/// A user's inotify instances are limited by fs.inotify.max_user_instances.
#[test]
fn instances_are_limited_per_user() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output(&out, "inotifylimit", &[]);
}
