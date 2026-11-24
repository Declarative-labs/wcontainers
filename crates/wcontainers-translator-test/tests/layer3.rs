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
use wcontainers_arm64::Arm64;
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::jobs::at_once;
use wcontainers_translator_test::layer3::{
    Compiler, LAYER3_CASE_STRIDE, LEVELS, Position, build, csmith_case, native_in_time, ordinary,
};
use wcontainers_translator_test::mixed::mixed;
use wcontainers_translator_test::programs::{
    Blob, Ran, SIGNALLED, check_ordinary_programs, native, native_program, ran as outcome,
    reachable_memory, ready, translated_for, translated_stopping_everywhere,
};
use wcontainers_x86::X86_64;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// The signals a program whose access or jump went astray ends by, depending on what lies where it went.
/// A stray access raises SIGSEGV or SIGBUS (no page, or a file's page past the file's end, which an
/// ordinary executable has and a blob does not). A stray jump into data faults in an ordinary executable,
/// whose data cannot be run, but runs the data in a blob, which is all one region that can be (see
/// `placement`), and ends by whatever the data decodes to: an undefined instruction, a breakpoint or a
/// stray access.
const ASTRAY: [u32; 4] = [SIGSEGV, SIGBUS, SIGILL, SIGTRAP];

/// Whether two exit statuses end the same way, every signal of a program gone astray being one.
fn same_ending(one: i32, other: i32) -> bool {
    let astray = |status: i32| {
        u32::try_from(status - SIGNALLED).is_ok_and(|signal| ASTRAY.contains(&signal))
    };
    one == other || (astray(one) && astray(other))
}

#[test]
fn a_program_gone_astray_ends_the_same_by_any_of_its_signals() {
    let status = |signal: u32| SIGNALLED + signal as i32;
    for one in ASTRAY {
        for other in ASTRAY {
            assert!(same_ending(status(one), status(other)));
        }
        assert!(!same_ending(status(one), 0));
        assert!(!same_ending(status(one), status(SIGKILL)));
        assert!(!same_ending(status(one), status(SIGABRT)));
    }
    assert!(!same_ending(SIGBUS as i32, SIGSEGV as i32));
}

/// Run a blob at none and full in both settings; compare with Linux, and full with none in registers and
/// memory. Gives Linux's run, or nothing when the program did not end in time there. `ordinary` runs the
/// program built as an ordinary Linux executable.
fn compare<G: GuestTest>(blob: &Blob, ordinary: impl Fn() -> Ran) -> Result<Option<Ran>, String> {
    let Some(theirs) = native_in_time::<G>(blob) else {
        return Ok(None);
    };
    // A blob that Linux ends by a signal is compared only when the program, built as an ordinary
    // executable, ends the same way (`same_ending`): otherwise both sides could agree on a blob that is broken (a
    // moved blob that did not relocate itself, say). What it wrote before may differ, as a program that
    // faults may have read through a wild pointer, whose target depends on where things are.
    if theirs.status >= SIGNALLED {
        let program = ordinary();
        if !same_ending(program.status, theirs.status) {
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
        use std::hash::Hasher;
        let mut hasher = rustc_hash::FxHasher::default();
        hasher.write(&blob.bytes);
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

fn csmith_programs_match_linux<G: GuestTest>() {
    let cases = setting("WC_LAYER3_CASES", 4);
    let seed = setting("WC_LAYER3_SEED", 1);
    let from = setting("WC_LAYER3_FROM", 0);
    let chrome = setting("WC_LAYER3_CHROME", 1);
    let jobs = setting("WC_LAYER3_JOBS", 1).max(1);
    let chrome_every = cases.div_ceil(chrome.max(1));
    let in_chrome = AtomicU64::new(0);
    let (ran, too_long, done) = (AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0));
    // Programs whose floating point runs on the exact path: they read FPSR.
    let exact = AtomicU64::new(0);
    let failures = Mutex::new(Vec::new());
    at_once(jobs, cases, |i| {
        let number = seed.wrapping_mul(LAYER3_CASE_STRIDE).wrapping_add(from + i);
        let (source, compiler, level, position) = csmith_case(number);
        let blob = build::<G>(&source, compiler, level, position);
        if blob.reads_float_status::<G>() {
            exact.fetch_add(1, Ordering::Relaxed);
        }
        let own = || ordinary::<G>(&source, compiler, level, position);
        let checked = compare::<G>(&blob, own).and_then(|theirs| match theirs {
            Some(theirs) if chrome > 0 && i.is_multiple_of(chrome_every) => {
                in_chrome.fetch_add(1, Ordering::Relaxed);
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
        "layer 3: {cases} Csmith programs from case {from}, seed {seed}, {jobs} at once: {} compared ({} also in Chrome), {} did not end in time on Linux, {} read FPSR, {} failing: {failures:?}",
        ran.into_inner(),
        in_chrome.into_inner(),
        too_long.into_inner(),
        exact.into_inner(),
        failures.len()
    );
    assert!(failures.is_empty());
}

/// The hand-picked programs `names` with the C library (number printing, sorting and, for ARM64, the
/// differences of section 8), with both compilers, every level and both positions.
fn hand_picked_programs_match_linux<G: GuestTest>(names: &[&str]) {
    let mut failures = Vec::new();
    for &name in names {
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
    hand_picked_programs_match_linux::<Arm64>(&["hello", "differences"]);
}

#[test]
fn ordinary_executables_run_on_the_reference_x86_64() {
    ordinary_executables_run_on_the_reference::<X86_64>();
}

#[test]
fn csmith_programs_match_linux_x86_64() {
    csmith_programs_match_linux::<X86_64>();
}

#[test]
fn hand_picked_programs_match_linux_x86_64() {
    hand_picked_programs_match_linux::<X86_64>(&["hello"]);
}

/// The native output for a host process (docs/x86-native-output.md section 8): Csmith's programs as above, at
/// none, at a bound picked from the blob and at full, without and with the count, linked and run on this machine
/// against Linux there, and the count after each system call against the WebAssembly output's. Linux x86-64
/// only, where the reference is this machine. Knobs as above, but Chrome's.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn csmith_programs_match_linux_natively_x86_64() {
    use wcontainers_translator_test::guest::x86_64::native_programs::compare;
    use wcontainers_translator_test::progress::run_counting;
    use wcontainers_translator_test::reference::with_reference;
    let cases = setting("WC_LAYER3_CASES", 4);
    let seed = setting("WC_LAYER3_SEED", 1);
    let from = setting("WC_LAYER3_FROM", 0);
    let jobs = setting("WC_LAYER3_JOBS", 1).max(1);
    let [ran, too_long, runs, pieces] = std::array::from_fn(|_| AtomicU64::new(0));
    let count = |counter: &AtomicU64, n: u64| counter.fetch_add(n, Ordering::Relaxed);
    let failures = Mutex::new(Vec::new());
    at_once(jobs, cases, |i| {
        let number = seed.wrapping_mul(LAYER3_CASE_STRIDE).wrapping_add(from + i);
        let (source, compiler, level, position) = csmith_case(number);
        let blob = build::<X86_64>(&source, compiler, level, position);
        let checked = (|| -> Result<(), String> {
            let Some(theirs) = native_in_time::<X86_64>(&blob) else {
                count(&too_long, 1);
                return Ok(());
            };
            let counted = run_counting::<X86_64>(&blob, (LookAhead::None, Optimise::Speed));
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
            for look_ahead in [LookAhead::None, LookAhead::Bounded(bound), LookAhead::Full] {
                for counting in [false, true] {
                    let ours = with_reference(&X86_64::REFERENCE, |reference| {
                        compare(
                            reference,
                            &blob,
                            (wcontainers_x86::native::Way::Process, look_ahead),
                            counting,
                            &theirs,
                            &counted,
                        )
                    })
                    .map_err(|e| format!("{}: {e}", blob.name))?;
                    count(&runs, ours.runs.into());
                    count(&pieces, ours.pieces as u64);
                }
            }
            count(&ran, 1);
            Ok(())
        })();
        if let Err(report) = checked {
            eprintln!("csmith {number} (case {}) natively: {report}", from + i);
            failures.lock().unwrap().push(number);
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    let [ran, too_long, runs, pieces] = [ran, too_long, runs, pieces].map(|c| c.into_inner());
    eprintln!(
        "layer 3 natively: {cases} Csmith programs from case {from}, seed {seed}, {jobs} at once: {ran} compared at 3 look-aheads with and without the count, {too_long} did not end in time on Linux; {runs} blob runs, {pieces} pieces; {} failing: {failures:?}",
        failures.len()
    );
    assert!(failures.is_empty());
}
