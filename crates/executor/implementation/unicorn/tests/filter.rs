//! A program's own seccomp filters, compared with real ARM64 Linux in a default container.

use std::path::Path;

use wcontainers_executor_unicorn::{guest, run_both};

#[test]
fn filters_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("filter", &[], &out);
    // The last calls of each ending kill the program (SIGSYS, which Docker reports as 159) or run it
    // again.
    for (ending, exit_code) in [
        ("", 0),
        ("trap", 159),
        ("trapignored", 159),
        ("killprocess", 159),
        ("killthread", 159),
        ("unknownaction", 159),
        ("divzero", 159),
        ("execve", 0),
        ("limit", 0),
    ] {
        let arguments: &[&str] = if ending.is_empty() { &[] } else { &[ending] };
        let (ours, theirs) = run_both(&program, arguments);
        let ours_text = String::from_utf8_lossy(&ours.outcome.stdout).into_owned();
        let theirs_text = String::from_utf8_lossy(&theirs.stdout).into_owned();
        let differences: Vec<String> = ours_text
            .lines()
            .zip(theirs_text.lines())
            .filter(|(our, their)| our != their)
            .map(|(our, their)| format!("ours {our:?} theirs {their:?}"))
            .collect();
        assert!(
            differences.is_empty(),
            "{ending}: {}",
            differences.join("\n")
        );
        assert_eq!(ours.outcome, theirs, "{ending}");
        assert_eq!(theirs.exit_code, exit_code, "{ending}");
        assert!(!theirs_text.is_empty(), "{ending}");
    }
}
