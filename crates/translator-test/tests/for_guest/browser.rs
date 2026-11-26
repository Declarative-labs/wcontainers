// Written once for both guests; each guest's module in `browser.rs` includes it.

/// Each program, at every optimisation level, translated each way. Levels that build to the same bytes as an
/// earlier one are run once.
pub(super) fn programs_match_linux_in_chrome(
    programs: &[(&str, Library)],
    ways: &[(LookAhead, Optimise)],
) {
    programs_told_match_linux_in_chrome(programs, ways, None);
}
/// As `programs_match_linux_in_chrome`, for programs told the offer whose key is `offer`.
pub(super) fn programs_told_match_linux_in_chrome(
    programs: &[(&str, Library)],
    ways: &[(LookAhead, Optimise)],
    offer: Option<u64>,
) {
    for &(name, library) in programs {
        let mut built: Vec<Vec<u8>> = Vec::new();
        for optimisation in LEVELS {
            let mut blob = build(name, optimisation, library);
            blob.offer = offer;
            if built.contains(&blob.bytes) {
                continue;
            }
            built.push(blob.bytes.clone());
            let theirs = native(&blob);
            for &(look_ahead, optimise) in ways {
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
