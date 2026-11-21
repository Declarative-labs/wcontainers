//! I/O signals for pipes and terminals compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;

#[test]
fn io_signals_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "sigio", &[]);
}

/// A pseudo-terminal master given FIOASYNC with no owner named, signalled by a write to its slave.
#[test]
fn terminal_signals_without_an_owner_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "ptyasync", &[]);
}
