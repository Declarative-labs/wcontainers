//! Layer 3 against real ARM64 Linux: Csmith's random programs and the hand-picked ones, built with clang
//! and gcc at every optimisation level, of fixed position and position independent code, at look-ahead none
//! and full (for speed and for size, plain and with the thread stopping at every stop check), at bounded
//! look-ahead of an N picked from the blob, and with pieces of all three mixed and events injected (see
//! `mixed`).
//!
//! `WC_LAYER3_CASES` Csmith programs (default 4), seeded from `WC_LAYER3_SEED` (default 1): case `i` is
//! Csmith seed `seed * 1_000_003 + i`, with the compiler, level and position picked from the same number,
//! so a failure replays from the two values: `WC_LAYER3_FROM=i` (default 0) starts at case `i`. The first
//! `WC_LAYER3_CHROME` of them that end in time (default one) also run in Chrome, at none and full in both
//! settings.
//! `WC_LAYER3_JOBS` cases run at once (default one). A line every thousand cases shows progress.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use wcontainers_arm64::Arm64;
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::jobs::at_once;
use wcontainers_translator_test::layer3::{
    Compiler, LEVELS, Position, build, csmith, native_in_time, ordinary,
};
use wcontainers_translator_test::mixed::mixed;
use wcontainers_translator_test::programs::{
    Blob, Ran, check_ordinary_programs, native, native_program, ran as outcome, reachable_memory,
    ready, translated_for, translated_stopping_everywhere,
};

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// Run a blob at none and full in both settings; compare with Linux, and full with none in registers and
/// memory. Gives Linux's run, or nothing when the program did not end in time there. `ordinary` runs the
/// program built as an ordinary Linux executable.
fn compare<G: GuestTest>(blob: &Blob, ordinary: impl Fn() -> Ran) -> Result<Option<Ran>, String> {
    let Some(theirs) = native_in_time::<G>(blob) else {
        return Ok(None);
    };
    // A blob that Linux ends by a signal is compared only when the program, built as an ordinary
    // executable, ends by the same signal: otherwise both sides could agree on a blob that is broken (a
    // moved blob that did not relocate itself, say). What it wrote before may differ, as a program that
    // faults may have read through a wild pointer, whose target depends on where things are.
    if theirs.status >= 128 {
        let program = ordinary();
        if program.status != theirs.status {
            // Whether Linux does it again tells a fault of the blob from one of the reference.
            let again: Vec<i32> = (0..3).map(|_| native::<G>(blob).status).collect();
            return Err(format!(
                "{}: Linux ended the blob with status {}, then {again:?} when run again, the ordinary executable with status {}",
                blob.name, theirs.status, program.status
            ));
        }
    }
    let mut finals = Vec::new();
    for (look_ahead, optimise, stopping) in [
        (LookAhead::None, Optimise::Speed, false),
        (LookAhead::Full, Optimise::Speed, false),
        (LookAhead::Full, Optimise::Size, false),
        (LookAhead::Full, Optimise::Speed, true),
        (LookAhead::Full, Optimise::Size, true),
    ] {
        let how = (look_ahead, optimise);
        let (ours, mut machine, registers) = if stopping {
            translated_stopping_everywhere::<G>(blob, how)
        } else {
            translated_for::<G>(blob, how)
        };
        if ours != theirs {
            return Err(format!(
                "{} at {look_ahead:?} for {optimise:?}, stopping everywhere {stopping}: ours {:?} status {}, Linux {:?} status {}",
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
        finals.push((registers, reachable_memory::<G>(&mut machine, blob)));
    }
    if finals[1..].iter().any(|full| *full != finals[0]) {
        return Err(format!("{}: full differs from none at the end", blob.name));
    }
    // Bounded of an N and pieces mixed with events injected, both from the blob's own hash.
    let seed = {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        blob.bytes.hash(&mut hasher);
        hasher.finish()
    };
    let bound = 2 + (seed % 63) as u32;
    let look_ahead = LookAhead::Bounded(bound);
    let mut machine = ready::<G>(blob, look_ahead, Default::default());
    machine.translate_at_run_time(move || look_ahead);
    let (stopped, registers) = machine.run();
    let ours = outcome::<G>(blob, &machine, stopped, &registers);
    if ours != theirs || (registers, reachable_memory::<G>(&mut machine, blob)) != finals[0] {
        return Err(format!(
            "{} at bounded {bound} differs from none",
            blob.name
        ));
    }
    let (ours, mut machine, registers, events) = mixed::<G>(blob, seed);
    if ours != theirs || (registers, reachable_memory::<G>(&mut machine, blob)) != finals[0] {
        return Err(format!(
            "{} mixed from {seed} ({events:?}) differs from none",
            blob.name
        ));
    }
    Ok(Some(theirs))
}

/// Run a blob in Chrome at none and at full for speed and for size, and compare with Linux's run.
fn compare_in_chrome<G: GuestTest>(blob: &Blob, theirs: &Ran) -> Result<(), String> {
    for how in [
        (LookAhead::None, Optimise::Speed),
        (LookAhead::Full, Optimise::Speed),
        (LookAhead::Full, Optimise::Size),
    ] {
        let ours = browser::translated_with::<G>(blob, how, Omit::default());
        if ours.ran != *theirs {
            return Err(format!(
                "{} in Chrome at {how:?}: ours {:?} status {}, Linux {:?} status {}",
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

/// The source, compiler, level and position of Csmith case `number`.
fn csmith_case(number: u64) -> (std::path::PathBuf, Compiler, &'static str, Position) {
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
    (csmith(number), compiler, level, position)
}

fn csmith_programs_match_linux<G: GuestTest>() {
    let cases = setting("WC_LAYER3_CASES", 4);
    let seed = setting("WC_LAYER3_SEED", 1);
    let from = setting("WC_LAYER3_FROM", 0);
    let chrome = setting("WC_LAYER3_CHROME", 1);
    let jobs = setting("WC_LAYER3_JOBS", 1).max(1);
    let in_chrome = AtomicU64::new(0);
    let (ran, too_long, done) = (AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0));
    let failures = Mutex::new(Vec::new());
    at_once(jobs, cases, |i| {
        let number = seed.wrapping_mul(1_000_003).wrapping_add(from + i);
        let (source, compiler, level, position) = csmith_case(number);
        let blob = build::<G>(&source, compiler, level, position);
        let own = || ordinary::<G>(&source, compiler, level, position);
        let checked = compare::<G>(&blob, own).and_then(|theirs| match theirs {
            Some(theirs) if in_chrome.fetch_add(1, Ordering::Relaxed) < chrome => {
                compare_in_chrome::<G>(&blob, &theirs).map(|()| Some(theirs))
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
fn hand_picked_programs_match_linux<G: GuestTest>() {
    let mut failures = Vec::new();
    for name in ["hello", "differences"] {
        for compiler in [Compiler::Clang, Compiler::Gcc] {
            for level in LEVELS {
                for position in [Position::Fixed, Position::Independent] {
                    let source = wcontainers_translator_test::programs::source::<G>(name);
                    let blob = build::<G>(&source, compiler, level, position);
                    let own = || ordinary::<G>(&source, compiler, level, position);
                    match compare::<G>(&blob, own) {
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

/// The reference runs ordinary executables, which tell a blob's signal ending to be the program's own.
fn ordinary_executables_run_on_the_reference<G: GuestTest>() {
    check_ordinary_programs(G::C_TARGET, native_program::<G>);
}

#[test]
fn ordinary_executables_run_on_the_reference_arm64() {
    ordinary_executables_run_on_the_reference::<Arm64>();
}

#[test]
fn csmith_programs_match_linux_arm64() {
    csmith_programs_match_linux::<Arm64>();
}

#[test]
fn hand_picked_programs_match_linux_arm64() {
    hand_picked_programs_match_linux::<Arm64>();
}
