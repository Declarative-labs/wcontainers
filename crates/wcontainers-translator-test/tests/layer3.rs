//! Layer 3 against real ARM64 Linux: Csmith's random programs and the hand-picked ones, built with clang
//! and gcc at every optimisation level, of fixed position and position independent code, at look-ahead none
//! and full.
//!
//! `WC_LAYER3_CASES` Csmith programs (default 4), seeded from `WC_LAYER3_SEED` (default 1): case `i` is
//! Csmith seed `seed * 1_000_003 + i`, with the compiler, level and position picked from the same number,
//! so a failure replays from the two values.

use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::layer3::{
    Compiler, LEVELS, Position, build, csmith, native_in_time,
};
use wcontainers_translator_test::programs::{Blob, reachable_memory, translated};

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// Run a blob at none and full; compare with Linux, and full with none in registers and memory.
fn compare(blob: &Blob) -> Result<bool, String> {
    let Some(theirs) = native_in_time(blob) else {
        return Ok(false);
    };
    // Every program here ends normally, so ending by a signal (a moved blob that did not relocate itself,
    // say) would make agreement meaningless.
    if theirs.status >= 128 {
        return Err(format!(
            "{}: Linux ended with status {}",
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
    Ok(true)
}

#[test]
fn csmith_programs_match_linux() {
    let cases = setting("WC_LAYER3_CASES", 4);
    let seed = setting("WC_LAYER3_SEED", 1);
    let (mut ran, mut too_long, mut failures) = (0, 0, Vec::new());
    for i in 0..cases {
        let number = seed.wrapping_mul(1_000_003).wrapping_add(i);
        let source = csmith(number);
        let compiler = if number % 2 == 0 {
            Compiler::Clang
        } else {
            Compiler::Gcc
        };
        let level = LEVELS[(number / 2 % LEVELS.len() as u64) as usize];
        let position = if number / 12 % 2 == 0 {
            Position::Fixed
        } else {
            Position::Independent
        };
        let blob = build(&source, compiler, level, position);
        match compare(&blob) {
            Ok(true) => ran += 1,
            Ok(false) => too_long += 1,
            Err(report) => {
                eprintln!("csmith {number}: {report}");
                failures.push(number);
            }
        }
    }
    eprintln!(
        "layer 3: {cases} Csmith programs, seed {seed}: {ran} compared, {too_long} did not end in time on Linux, {} failing: {failures:?}",
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
                        Ok(true) => {}
                        Ok(false) => failures.push(format!("{}: did not end on Linux", blob.name)),
                        Err(report) => failures.push(report),
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
