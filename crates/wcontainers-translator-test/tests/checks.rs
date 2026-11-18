//! Checks that cover several accesses at once (a stack frame, a group through one base) failing, also on the
//! first instruction of a function: the same result as Linux, and at full the same registers and memory as at
//! none. The programs are each guest's own (`GuestTest::checks`).

use wcontainers_arm64::Arm64;
use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::guest::{Check, Counted, GuestTest};
use wcontainers_translator_test::programs::{
    Blob, native, reachable_memory, translated, translated_omitting,
};
use wcontainers_translator_test::runner::Machine;

/// Linux, none and full (translated for speed and for size) agree, and full ends with the registers and
/// memory of none. Returns the machines that ran at full, for their counters.
fn compare<G: GuestTest>(blob: &Blob) -> [Machine<G>; 2] {
    let theirs = native::<G>(blob);
    let (none, mut none_machine, none_registers) = translated::<G>(blob, LookAhead::None);
    assert_eq!(none, theirs, "{}: none against Linux", blob.name);
    let none_memory = reachable_memory::<G>(&mut none_machine, blob);
    [Optimise::Speed, Optimise::Size].map(|optimise| {
        let how = (LookAhead::Full, optimise);
        let (full, mut full_machine, full_registers, _) =
            translated_omitting::<G>(blob, how, Omit::default());
        eprintln!(
            "{} for {optimise:?}: status {}, full counters {:?}",
            blob.name,
            theirs.status,
            full_machine.counters()
        );
        assert_eq!(
            full, theirs,
            "{} for {optimise:?}: full against Linux",
            blob.name
        );
        assert_eq!(full_registers, none_registers, "{}: registers", blob.name);
        assert!(
            reachable_memory::<G>(&mut full_machine, blob) == none_memory,
            "{} for {optimise:?}: memory",
            blob.name
        );
        full_machine
    })
}

/// Whether a counter read `count` as the check says it must.
fn counted_as(count: u64, counted: Counted) -> bool {
    match counted {
        Counted::Exactly(n) => count == n,
        Counted::Some => count > 0,
    }
}

/// Every check against Linux, none and full, each on a thread of its own as separate tests would be.
fn checks_match_linux<G: GuestTest>() {
    let checks = G::checks();
    std::thread::scope(|scope| {
        for check in &checks {
            scope.spawn(move || {
                let Check { blob, counted, .. } = check;
                for full in compare::<G>(blob) {
                    if let Some((reason, counted)) = *counted {
                        let count = full.count(reason);
                        assert!(
                            counted_as(count, counted),
                            "{}: {} counted {count} times, must be {counted:?}",
                            blob.name,
                            reason.name()
                        );
                    }
                }
            });
        }
    });
    eprintln!("checks: {} programs", checks.len());
}

/// The checks that fail on a function's first instruction, at full in Chrome, for speed and for size.
fn failing_at_an_entry_in_chrome<G: GuestTest>() {
    for check in G::checks().iter().filter(|check| check.at_entry) {
        let blob = &check.blob;
        let theirs = native::<G>(blob);
        for optimise in [Optimise::Speed, Optimise::Size] {
            let how = (LookAhead::Full, optimise);
            let ours = browser::translated_with::<G>(blob, how, Omit::default());
            eprintln!("{} for {optimise:?}: {:?}", blob.name, ours.counters);
            assert_eq!(ours.ran, theirs, "{} for {optimise:?} in Chrome", blob.name);
            if let Some((reason, counted)) = check.counted {
                let count = ours.count(reason.name());
                assert!(
                    counted_as(count, counted),
                    "{} in Chrome: {} counted {count} times, must be {counted:?}",
                    blob.name,
                    reason.name()
                );
            }
        }
    }
}

#[test]
fn checks_match_linux_arm64() {
    checks_match_linux::<Arm64>();
}

#[test]
fn failing_at_an_entry_in_chrome_arm64() {
    failing_at_an_entry_in_chrome::<Arm64>();
}
