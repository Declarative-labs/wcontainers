//! The signal fields of proc's status and stat files, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_executor_unicorn::{guest, run_both};

#[test]
fn signal_fields_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("procsignals", &[], &out);
    let (ours, theirs) = run_both(&program, &[]);
    let ours_text = String::from_utf8_lossy(&ours.outcome.stdout).into_owned();
    let theirs_text = String::from_utf8_lossy(&theirs.stdout).into_owned();
    let differences: Vec<String> = ours_text
        .lines()
        .zip(theirs_text.lines())
        .filter(|(our, their)| our != their)
        .map(|(our, their)| format!("ours {our:?} theirs {their:?}"))
        .collect();
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    assert_eq!(ours.outcome, theirs);
}

/// A continued child's SIGCHLD comes when the child next runs, not inside the sender's kill.
#[test]
fn continued_child_tells_its_parent_when_it_runs() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output(&out, "continued", &[]);
}

/// Calls blocked in a stopped process go on when it is continued, saved and brought back while
/// stopped or not.
#[test]
fn stopped_calls_go_on_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    wcontainers_executor_unicorn::assert_same_output_seeded(&out, "stopread", &[], 6);
}

/// SIGUSR1 sent from outside at moments drawn from each seed, into work written so that what it
/// prints does not depend on the signal (plan section 13's injected signals), with saves too; the
/// reference runs it with no signal sent.
#[test]
fn injected_signals_change_nothing() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_executor_unicorn::guest::build("interrupted", &[], &out);
    let theirs = wcontainers_executor_unicorn::reference::run(&program, &[]);
    for seed in wcontainers_executor_unicorn::schedule_seeds() {
        let injections = wcontainers_executor_unicorn::driver::Injections {
            saves: 3,
            signal: Some((wcontainers_kernel_linux::linux::general::SIGUSR1, 40)),
            ..Default::default()
        };
        let ours = wcontainers_executor_unicorn::run_ours_injected(
            program.parent().expect("a directory"),
            "/wc/interrupted",
            &["/wc/interrupted"],
            seed,
            injections,
        );
        assert_eq!(ours.outcome, theirs, "seed {seed}");
        assert!(ours.warnings.is_empty(), "{:?}", ours.warnings);
    }
}
