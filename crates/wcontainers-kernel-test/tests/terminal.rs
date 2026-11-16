//! Pseudo-terminals: the pair, the line discipline, output processing, flow control, packet mode,
//! closing and job control, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel::reason;
use wcontainers_kernel_test::assert_same_output;

fn compare(name: &str, arguments: &[&str]) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let ours = assert_same_output(&out, name, arguments);
    // `/proc/self/stat` is read for its terminal fields; the fields it does not track are others.
    // TIOCSETD asks for disciplines the reference has not registered either.
    assert!(
        ours.warnings.iter().all(|warned| [
            reason::PROC_FIELDS_NOT_TRACKED,
            reason::LDISC_NOT_BUILT
        ]
        .contains(&warned.reason.as_str())),
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
fn waiting_for_room() {
    compare("tty", &["waiting"]);
}

#[test]
fn job_control() {
    compare("jobs", &[]);
}

/// The image's shell with its job control on a terminal: jobs stopped and continued, background
/// reads and writes stopped, and keys that interrupt.
#[test]
fn interactive_shell() {
    compare("shpty", &[]);
}
