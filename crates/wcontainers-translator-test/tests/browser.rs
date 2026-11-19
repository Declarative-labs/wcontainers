//! C programs give the same output and exit status in Chrome, translated at look-ahead none and full, and at
//! bounded look-ahead from the first instruction with every other piece added while the program runs, for
//! speed and for size, as on real ARM64 Linux.

use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::programs::{Library, build, native};

const PROGRAMS: [(&str, Library); 7] = [
    ("hello", Library::C),
    ("differences", Library::C),
    ("bench", Library::None),
    ("integers", Library::None),
    ("tables", Library::None),
    ("buffers", Library::None),
    ("walks", Library::None),
];
const LEVELS: [&str; 5] = ["-O0", "-O1", "-O2", "-O3", "-Os"];

/// Every program, at every optimisation level. Levels that build to the same bytes as an earlier one are
/// run once.
#[test]
fn programs_match_linux_in_chrome() {
    for (name, library) in PROGRAMS {
        let mut built: Vec<Vec<u8>> = Vec::new();
        for optimisation in LEVELS {
            let blob = build(name, optimisation, library);
            if built.contains(&blob.bytes) {
                continue;
            }
            built.push(blob.bytes.clone());
            let theirs = native(&blob);
            for (look_ahead, optimise) in [
                (LookAhead::None, Optimise::Speed),
                (LookAhead::Full, Optimise::Speed),
                (LookAhead::Full, Optimise::Size),
                (LookAhead::AT_RUN_TIME, Optimise::Speed),
                (LookAhead::AT_RUN_TIME, Optimise::Size),
            ] {
                let how = (look_ahead, optimise);
                let ours = browser::translated_with(&blob, how, Omit::default());
                let what =
                    format!("{name} at {optimisation}, look-ahead {look_ahead:?} for {optimise:?}");
                assert_eq!(
                    String::from_utf8_lossy(&ours.ran.stdout),
                    String::from_utf8_lossy(&theirs.stdout),
                    "output of {what}"
                );
                assert_eq!(ours.ran.stdout, theirs.stdout, "output bytes of {what}");
                assert_eq!(
                    String::from_utf8_lossy(&ours.ran.stderr),
                    String::from_utf8_lossy(&theirs.stderr),
                    "error output of {what}"
                );
                assert_eq!(ours.ran.status, theirs.status, "exit status of {what}");
                assert_eq!(ours.count("unsupported instruction reached"), 0);
                if look_ahead == LookAhead::AT_RUN_TIME {
                    assert!(ours.count("translated at run time") > 0, "{what}");
                }
                eprintln!(
                    "{what}: matches in {}, {:?}",
                    ours.user_agent, ours.counters
                );
            }
        }
    }
}
