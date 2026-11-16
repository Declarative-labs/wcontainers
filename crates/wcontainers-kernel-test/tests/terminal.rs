//! Pseudo-terminals: the pair, the line discipline, output processing, flow control, packet mode,
//! closing and job control, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::assert_same_output;

fn compare(name: &str, arguments: &[&str]) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, name, arguments);
    // `/proc/self/stat` is read for its terminal fields; the fields it does not track are others.
    assert!(
        ours.warnings
            .iter()
            .all(|warned| warned.reason == reason::PROC_FIELDS_NOT_TRACKED),
        "{:?}",
        ours.warnings
    );
}

#[test]
fn making_pairs() {
    compare("tty", &["making"]);
}

#[test]
fn line_editing() {
    compare("tty", &["editing"]);
}

#[test]
fn non_canonical_reads() {
    compare("tty", &["raw"]);
}

#[test]
fn flow_control_and_size() {
    compare("tty", &["flow"]);
}

#[test]
fn closing_sides() {
    compare("tty", &["closing"]);
}

#[test]
fn job_control() {
    compare("jobs", &[]);
}
