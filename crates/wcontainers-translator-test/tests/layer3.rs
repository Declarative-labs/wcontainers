//! Layer 3 against real ARM64 Linux: Csmith's random programs and the hand-picked ones, built with clang
//! and gcc at every optimisation level, of fixed position and position independent code, at look-ahead none
//! and full.
//!
//! `WC_LAYER3_CASES` Csmith programs (default 4), seeded from `WC_LAYER3_SEED` (default 1): case `i` is
//! Csmith seed `seed * 1_000_003 + i`, with the compiler, level and position picked from the same number,
//! so a failure replays from the two values: `WC_LAYER3_FROM=i` (default 0) starts at case `i`. The first
//! `WC_LAYER3_CHROME` of them that end in time (default one) also run in Chrome, at none and full.
//! `WC_LAYER3_JOBS` cases run at once (default one). A line every thousand cases shows progress.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Omit};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::layer3::{
    Compiler, LEVELS, Position, build, csmith, native_in_time,
};
use wcontainers_translator_test::programs::{Blob, Ran, native, reachable_memory, translated};

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// Run a blob at none and full; compare with Linux, and full with none in registers and memory. Gives
/// Linux's run, or nothing when the program did not end in time there.
fn compare(blob: &Blob) -> Result<Option<Ran>, String> {
    let Some(theirs) = native_in_time(blob) else {
        return Ok(None);
    };
    // Every program here ends normally, so ending by a signal (a moved blob that did not relocate itself,
    // say) would make agreement meaningless.
    if theirs.status >= 128 {
        // Whether Linux does it again tells a fault of the program from one of the reference.
        let again: Vec<i32> = (0..3).map(|_| native(blob).status).collect();
        return Err(format!(
            "{}: Linux ended with status {}, then {again:?} when run again",
            blob.name, theirs.status
        ));
    }
    let mut finals = Vec::new();
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let (ours, mut machine, registers) = translated(blob, look_ahead);
        if ours != theirs {
            return Err(format!(
                "{} at {look_ahead:?}: ours {:?} status {}, Linux {:?} status {}",
                blob.name,
                String::from_utf8_lossy(&ours.stdout),
                ours.status,
                String::from_utf8_lossy(&theirs.stdout),
                theirs.status
            ));
        }
        if machine.count(Reason::UnsupportedInstructionReached) != 0 {
            return Err(format!("{}: unsupported instruction reached", blob.name));
        }
        finals.push((registers, reachable_memory(&mut machine, blob)));
    }
    if finals[0] != finals[1] {
        return Err(format!("{}: full differs from none at the end", blob.name));
    }
    Ok(Some(theirs))
}

/// Run a blob in Chrome at none and full and compare with Linux's run.
fn compare_in_chrome(blob: &Blob, theirs: &Ran) -> Result<(), String> {
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let ours = browser::translated_with(blob, look_ahead, Omit::default());
        if ours.ran != *theirs {
            return Err(format!(
                "{} in Chrome at {look_ahead:?}: ours {:?} status {}, Linux {:?} status {}",
                blob.name,
                String::from_utf8_lossy(&ours.ran.stdout),
                ours.ran.status,
                String::from_utf8_lossy(&theirs.stdout),
                theirs.status
            ));
        }
    }
    Ok(())
}

/// The blob for Csmith case `number`, with the compiler, level and position picked from it.
fn csmith_blob(number: u64) -> Blob {
    let compiler = if number.is_multiple_of(2) {
        Compiler::Clang
    } else {
        Compiler::Gcc
    };
    let level = LEVELS[(number / 2 % LEVELS.len() as u64) as usize];
    let position = if (number / 12).is_multiple_of(2) {
        Position::Fixed
    } else {
        Position::Independent
    };
    build(&csmith(number), compiler, level, position)
}

#[test]
fn csmith_programs_match_linux() {
    let cases = setting("WC_LAYER3_CASES", 4);
    let seed = setting("WC_LAYER3_SEED", 1);
    let from = setting("WC_LAYER3_FROM", 0);
    let chrome = setting("WC_LAYER3_CHROME", 1);
    let jobs = setting("WC_LAYER3_JOBS", 1).max(1);
    let (next, in_chrome) = (AtomicU64::new(0), AtomicU64::new(0));
    let (ran, too_long, done) = (AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0));
    let failures = Mutex::new(Vec::new());
    let work = || {
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            if i >= cases {
                return;
            }
            let number = seed.wrapping_mul(1_000_003).wrapping_add(from + i);
            let blob = csmith_blob(number);
            let checked = compare(&blob).and_then(|theirs| match theirs {
                Some(theirs) if in_chrome.fetch_add(1, Ordering::Relaxed) < chrome => {
                    compare_in_chrome(&blob, &theirs).map(|()| Some(theirs))
                }
                other => Ok(other),
            });
            match checked {
                Ok(Some(_)) => ran.fetch_add(1, Ordering::Relaxed),
                Ok(None) => too_long.fetch_add(1, Ordering::Relaxed),
                Err(report) => {
                    eprintln!("csmith {number} (case {}): {report}", from + i);
                    failures.lock().unwrap().push(number);
                    0
                }
            };
            let done = done.fetch_add(1, Ordering::Relaxed) + 1;
            if done.is_multiple_of(1000) {
                let failing = failures.lock().unwrap().len();
                eprintln!("layer 3: {done} of {cases} done, {failing} failing so far");
            }
        }
    };
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(work);
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    eprintln!(
        "layer 3: {cases} Csmith programs from case {from}, seed {seed}, {jobs} at once: {} compared ({} also in Chrome), {} did not end in time on Linux, {} failing: {failures:?}",
        ran.into_inner(),
        in_chrome.into_inner().min(chrome),
        too_long.into_inner(),
        failures.len()
    );
    assert!(failures.is_empty());
}

/// The hand-picked programs with the C library (number printing, sorting and the differences of section
/// 8), with both compilers, every level and both positions.
#[test]
fn hand_picked_programs_match_linux() {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("programs");
    let mut failures = Vec::new();
    for name in ["hello", "differences"] {
        for compiler in [Compiler::Clang, Compiler::Gcc] {
            for level in LEVELS {
                for position in [Position::Fixed, Position::Independent] {
                    let blob = build(
                        &directory.join(format!("{name}.c")),
                        compiler,
                        level,
                        position,
                    );
                    match compare(&blob) {
                        Ok(Some(_)) => {}
                        Ok(None) => failures.push(format!("{}: did not end on Linux", blob.name)),
                        Err(report) => failures.push(report),
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
