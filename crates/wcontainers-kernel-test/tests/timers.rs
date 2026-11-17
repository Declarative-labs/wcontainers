//! Interval timers, POSIX timers and timer descriptors, compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output_seeded;

#[test]
fn timers_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    for run in assert_same_output_seeded(&out, "timers", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// Timers whose signal is ignored or blocked when they fire (`posixtimer_sig_ignore`,
/// `itimer_get_remtime`).
#[test]
fn ignored_and_blocked_timer_signals_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    for run in assert_same_output_seeded(&out, "timerignore", &[], 0) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

#[test]
fn cpu_clock_sleeps_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    for run in assert_same_output_seeded(&out, "cpusleep", &[], 2) {
        assert!(run.warnings.is_empty(), "{:?}", run.warnings);
    }
}

/// The wall clock set forward while timers and waits on it wait (the host's wall clock moves two
/// hours at each save, as across a suspend). The reference cannot be made to do this without
/// setting the clock of every container on the machine, so the lines are Linux's as its source
/// gives them (`clock_was_set`, `timerfd_clock_was_set`, `common_hrtimer_arm`).
#[test]
fn wall_clock_setting_moves_its_timers() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_kernel_test::guest::build("wallset", &[], &out);
    let expected = "\
absolute sleep on the wall clock: 0
futex wait until a wall clock time: -1 Operation timed out
waits ended before the hour on the monotonic clock: 1
canceled on set: -1 Operation canceled
canceled on set, read again: -1 Resource temporarily unavailable
set again: 0
absolute on the wall clock: 8 fired
relative on the wall clock: 8 fired
relative on the monotonic clock: -1 Resource temporarily unavailable
absolute POSIX timer fired: 1
relative POSIX timer fired: 0
relative POSIX timer has most of its hour left: 1
";
    const TWO_HOURS: u64 = 2 * 3600 * 1_000_000_000;
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let run = wcontainers_kernel_test::run_ours_wall_moved(
            program.parent().expect("a directory"),
            "/wc/wallset",
            &["/wc/wallset"],
            seed,
            20,
            TWO_HOURS,
        );
        assert_eq!(
            String::from_utf8_lossy(&run.outcome.stdout),
            expected,
            "seed {seed}"
        );
        assert!(run.saved > 0, "seed {seed}");
    }
}
