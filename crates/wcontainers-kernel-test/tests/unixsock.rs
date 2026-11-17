//! Local sockets compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn local_sockets_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "unixsock", &[]);
}

#[test]
fn urgent_bytes_and_io_signals_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "unixoob", &[]);
}

#[test]
fn process_descriptor_and_queue_options_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "sockpidfd", &[]);
}

#[test]
fn cycles_are_collected_as_linux() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "unixgc", &[]);
}
