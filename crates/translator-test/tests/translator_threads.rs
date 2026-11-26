//! The first translations of a process, made at once from many threads of another pool as layer 2 makes them, end.

use std::sync::mpsc;
use std::time::Duration;
use wcontainers_arm64::Arm64;
use wcontainers_guest_cpu::Guest;
use wcontainers_translator::{LookAhead, Optimise, ProgressCount, translate};
use wcontainers_translator_test::jobs::at_once;

/// How long the translations may take, many times what they need.
const PATIENCE: Duration = Duration::from_secs(120);
/// Threads of the other pool, and the translations they make, more than its threads so that some wait.
const JOBS: u64 = 8;
const TRANSLATIONS: u64 = 256;

#[test]
fn the_first_translations_from_another_pool_end() {
    let code = wcontainers_translator_test::arm64::part::smoke().exit;
    let host =
        Arm64::host_from_probe(wcontainers_translator_test::arm64::part::PROBE_ANSWERS[usize::MIN]);
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        at_once(JOBS, TRANSLATIONS, |_| {
            translate::<Arm64>(
                &code,
                0..code.len() as u64,
                None,
                &[u64::MIN],
                LookAhead::Full,
                Optimise::Speed,
                ProgressCount::Absent,
                wcontainers_translator::AccessOrdering::Unordered,
                host,
                (),
            )
            .expect("a translation");
        });
        done.send(()).unwrap();
    });
    finished
        .recv_timeout(PATIENCE)
        .expect("the translations end");
}
