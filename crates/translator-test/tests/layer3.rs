//! Layer 3 against real Linux on the guest's CPU: Csmith's random programs and the hand-picked ones, built with clang
//! and gcc at every optimisation level, of fixed position and position independent code, at look-ahead none
//! and full (for speed and for size, plain and with the thread stopping at every stop check), at bounded
//! look-ahead of an N picked from the blob, and with pieces of all three mixed and events injected (see
//! `mixed`).
//!
//! `WC_LAYER3_CASES` Csmith programs (default 4), seeded from `WC_LAYER3_SEED` (default 1): case `i` is
//! Csmith seed `seed * 1_000_003 + i`, with the compiler, level and position picked from the same number,
//! so a failure replays from the two values: `WC_LAYER3_FROM=i` (default 0) starts at case `i`. Of every
//! `WC_LAYER3_CASES / WC_LAYER3_CHROME` cases the first (default one in all) also runs in Chrome, at none and
//! full in both settings, when it ends in time. Which programs end in time depends on the reference's speed
//! (Linux's 20 s alarm); everything else replays exactly.
//! `WC_LAYER3_JOBS` cases run at once (default one). A line every thousand cases shows progress.

use linux_raw_sys::general::{SIGABRT, SIGBUS, SIGILL, SIGKILL, SIGSEGV, SIGTRAP};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::jobs::at_once;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

#[test]
fn a_program_gone_astray_ends_the_same_by_any_of_its_signals() {
    let status = |signal: u32| arm64::SIGNALLED + signal as i32;
    for one in arm64::ASTRAY {
        for other in arm64::ASTRAY {
            assert!(arm64::same_ending(status(one), status(other)));
        }
        assert!(!arm64::same_ending(status(one), 0));
        assert!(!arm64::same_ending(status(one), status(SIGKILL)));
        assert!(!arm64::same_ending(status(one), status(SIGABRT)));
    }
    assert!(!arm64::same_ending(SIGBUS as i32, SIGSEGV as i32));
}

#[test]
fn ordinary_executables_run_on_the_reference_arm64() {
    arm64::ordinary_executables_run_on_the_reference();
}

#[test]
fn csmith_programs_match_linux_arm64() {
    arm64::csmith_programs_match_linux();
}

#[test]
fn hand_picked_programs_match_linux_arm64() {
    arm64::hand_picked_programs_match_linux(&["hello", "differences"]);
}

#[test]
fn ordinary_executables_run_on_the_reference_x86_64() {
    x86_64::ordinary_executables_run_on_the_reference();
}

#[test]
fn csmith_programs_match_linux_x86_64() {
    x86_64::csmith_programs_match_linux();
}

#[test]
fn hand_picked_programs_match_linux_x86_64() {
    x86_64::hand_picked_programs_match_linux(&["hello"]);
}

/// The native output for a host process (docs/x86-native-output.md section 8): Csmith's programs as above, at
/// none, at a bound picked from the blob and at full, without and with the count, linked and run on this machine
/// against Linux there, and the count after each system call against the WebAssembly output's. Linux x86-64
/// only, where the reference is this machine. Knobs as above, but Chrome's.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn csmith_programs_match_linux_natively_x86_64() {
    csmith_natively_in(&[wcontainers_x86::native::Way::Process], "natively");
}

/// The same in the in-process way's two checked forms (docs/x86-native-output.md sections 4 and 8), the program and
/// its stack in the window. `WC_LAYER3_CHECK` takes one form (`wrapping` or `exact`), both otherwise.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn csmith_programs_match_linux_natively_in_process_x86_64() {
    use wcontainers_x86::native::{Check, Way};
    let only = std::env::var("WC_LAYER3_CHECK").ok();
    let ways: Vec<Way> = [("wrapping", Check::Wrapping), ("exact", Check::Exact)]
        .into_iter()
        .filter(|(name, _)| only.as_deref().is_none_or(|only| only == *name))
        .map(|(_, check)| Way::InProcess(check))
        .collect();
    csmith_natively_in(&ways, "natively in process");
}

/// Layer 3's Csmith programs in each of `ways`, the summary line starting "layer 3 " and `what`.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn csmith_natively_in(ways: &[wcontainers_x86::native::Way], what: &str) {
    use wcontainers_translator_test::reference::with_reference;
    use wcontainers_translator_test::x86_64::part::native_programs::compare;
    use wcontainers_translator_test::x86_64::progress::run_counting;
    let cases = setting("WC_LAYER3_CASES", 4);
    let seed = setting("WC_LAYER3_SEED", 1);
    let from = setting("WC_LAYER3_FROM", 0);
    let jobs = setting("WC_LAYER3_JOBS", 1).max(1);
    let [ran, too_long, runs, pieces] = std::array::from_fn(|_| AtomicU64::new(0));
    let count = |counter: &AtomicU64, n: u64| counter.fetch_add(n, Ordering::Relaxed);
    let failures = Mutex::new(Vec::new());
    at_once(jobs, cases, |i| {
        let number = seed
            .wrapping_mul(x86_64::LAYER3_CASE_STRIDE)
            .wrapping_add(from + i);
        let (source, compiler, level, position) = x86_64::csmith_case(number);
        let blob = x86_64::build(&source, compiler, level, position);
        let checked = (|| -> Result<(), String> {
            let Some(theirs) = x86_64::native_in_time(&blob) else {
                count(&too_long, 1);
                return Ok(());
            };
            let counted = run_counting(&blob, (LookAhead::None, Optimise::Speed));
            if counted.ran != theirs {
                return Err(format!(
                    "{}: the WebAssembly output differs from Linux",
                    blob.name
                ));
            }
            let bound = {
                use std::hash::Hasher;
                let mut hasher = rustc_hash::FxHasher::default();
                hasher.write(&blob.bytes);
                2 + (hasher.finish() % 63) as u32
            };
            for &way in ways {
                for look_ahead in [LookAhead::None, LookAhead::Bounded(bound), LookAhead::Full] {
                    for counting in [false, true] {
                        let ours = with_reference(
                            &wcontainers_translator_test::x86_64::part::REFERENCE,
                            |reference| {
                                compare(
                                    reference,
                                    &blob,
                                    (way, look_ahead),
                                    counting,
                                    &theirs,
                                    &counted,
                                )
                            },
                        )
                        .map_err(|e| format!("{}: {e}", blob.name))?;
                        count(&runs, ours.runs.into());
                        count(&pieces, ours.pieces as u64);
                    }
                }
            }
            count(&ran, 1);
            Ok(())
        })();
        if let Err(report) = checked {
            eprintln!("csmith {number} (case {}) {what}: {report}", from + i);
            failures.lock().unwrap().push(number);
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    let [ran, too_long, runs, pieces] = [ran, too_long, runs, pieces].map(|c| c.into_inner());
    eprintln!(
        "layer 3 {what} ({ways:?}): {cases} Csmith programs from case {from}, seed {seed}, {jobs} at once: {ran} compared at 3 look-aheads with and without the count, {too_long} did not end in time on Linux; {runs} blob runs, {pieces} pieces; {} failing: {failures:?}",
        failures.len()
    );
    assert!(failures.is_empty());
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    use wcontainers_translator_test::arm64 as harness;
    pub(super) use wcontainers_translator_test::arm64::browser;
    pub(super) use wcontainers_translator_test::arm64::layer3::{
        Compiler, LAYER3_CASE_STRIDE, LEVELS, Position, build, csmith_case, native_in_time,
        ordinary,
    };
    pub(super) use wcontainers_translator_test::arm64::mixed::mixed;
    use wcontainers_translator_test::arm64::part;
    pub(super) use wcontainers_translator_test::arm64::programs::{
        Blob, Ran, SIGNALLED, check_ordinary_programs, native, native_program, ran as outcome,
        reachable_memory, ready, translated_for, translated_stopping_everywhere,
    };

    include!("for_guest/layer3.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    use wcontainers_translator_test::x86_64 as harness;
    pub(super) use wcontainers_translator_test::x86_64::browser;
    pub(super) use wcontainers_translator_test::x86_64::layer3::{
        Compiler, LAYER3_CASE_STRIDE, LEVELS, Position, build, csmith_case, native_in_time,
        ordinary,
    };
    pub(super) use wcontainers_translator_test::x86_64::mixed::mixed;
    use wcontainers_translator_test::x86_64::part;
    pub(super) use wcontainers_translator_test::x86_64::programs::{
        Blob, Ran, SIGNALLED, check_ordinary_programs, native, native_program, ran as outcome,
        reachable_memory, ready, translated_for, translated_stopping_everywhere,
    };

    include!("for_guest/layer3.rs");
}
