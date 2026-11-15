//! C programs give the same output and exit status in Chrome, translated at look-ahead none and full, as on
//! real ARM64 Linux.

use wcontainers_translator::{LookAhead, Omit};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::programs::{Library, build, native};

const PROGRAMS: [(&str, Library); 5] = [
    ("hello", Library::C),
    ("bench", Library::None),
    ("integers", Library::None),
    ("tables", Library::None),
    ("buffers", Library::None),
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
            for look_ahead in [LookAhead::None, LookAhead::Full] {
                let ours = browser::translated_with(&blob, look_ahead, Omit::default());
                let what = format!("{name} at {optimisation}, look-ahead {look_ahead:?}");
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
                eprintln!(
                    "{what}: matches in {}, {:?}",
                    ours.user_agent, ours.counters
                );
            }
        }
    }
}
