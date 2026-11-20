//! The progress count is the same at every look-ahead, for speed and for size, and on every run; a stop
//! logged at one look-ahead is reached again exactly from each. Knobs: `WC_PROGRESS_SEED` (default 1) seeds
//! the bounded look-ahead's N and the limits; `WC_PROGRESS_STOPS` is how many limits a program and look-ahead
//! get (default 1).

use proptest::prelude::RngExt;
use wcontainers_translator::{BOUND, LookAhead, Optimise};
use wcontainers_translator_test::jobs::load;
use wcontainers_translator_test::programs::{Blob, Library, build_with, ready_for};
use wcontainers_translator_test::progress::{
    Counted, counting, every_check, finish, first_stop, reach, run_counting,
};
use wcontainers_translator_test::runner::Machine;
use wcontainers_translator_test::seeded::generator;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect(name))
}

/// Look-ahead none, bounded with the default N and with one drawn from `WC_PROGRESS_SEED`, and full, each for
/// speed and for size.
fn every_how(name: &str) -> Vec<(LookAhead, Optimise)> {
    let seed = setting("WC_PROGRESS_SEED", 1);
    let drawn = generator(seed, &[], name).random_range(2..=BOUND);
    let mut hows = Vec::new();
    for look_ahead in [
        LookAhead::None,
        LookAhead::Bounded(BOUND),
        LookAhead::Bounded(drawn),
        LookAhead::Full,
    ] {
        for optimise in [Optimise::Speed, Optimise::Size] {
            hows.push((look_ahead, optimise));
        }
    }
    hows
}

/// The programs checked: built at a level, with defines.
const PROGRAMS: [(&str, &str, Library, &[&str]); 8] = [
    ("progress_events", "-O0", Library::None, &["ROUNDS=5"]),
    ("leafcalls", "-O2", Library::C, &["SCALE=0"]),
    ("leafcalls", "-O0", Library::C, &["SCALE=0"]),
    ("bench", "-O0", Library::None, &["SCALE=0"]),
    ("bench", "-O2", Library::None, &["SCALE=0"]),
    ("jumps", "-O2", Library::C, &[]),
    ("tables", "-O2", Library::None, &[]),
    ("exceptions", "-O2", Library::C, &[]),
];

fn programs() -> impl Iterator<Item = Blob> {
    PROGRAMS
        .into_iter()
        .map(|(name, level, library, defines)| build_with(name, level, library, defines))
}

#[test]
fn the_count_of_a_small_program_is_its_events_counted_by_hand() {
    const ROUNDS: u64 = 5;
    // Each round a call, its return and two backward conditional jumps; then one jump through a register.
    const EVENTS: u64 = 4 * ROUNDS + 1;
    let blob = build_with(
        "progress_events",
        "-O0",
        Library::None,
        &[&format!("ROUNDS={ROUNDS}")],
    );
    for how in every_how(&blob.name) {
        let counted = run_counting(&blob, how);
        assert_eq!(counted.ran.status, 0, "at {how:?}");
        assert_eq!(counted.end, EVENTS, "at {how:?}");
    }
}

#[test]
fn the_count_is_the_same_at_every_look_ahead_and_on_every_run() {
    for blob in programs() {
        let mut first: Option<Counted> = None;
        for how in every_how(&blob.name) {
            // Twice each, so a count that differs from run to run shows.
            for _ in 0..2 {
                let counted = run_counting(&blob, how);
                match &first {
                    None => {
                        eprintln!(
                            "{}: count {} at the end, {} system calls",
                            blob.name,
                            counted.end,
                            counted.syscalls.len()
                        );
                        first = Some(counted);
                    }
                    Some(first) => assert_eq!(&counted, first, "{} at {how:?}", blob.name),
                }
            }
        }
    }
}

/// Every position where look-ahead bounded or full stops, stopping at every check, is one where none stops,
/// in the same order: none checks at every instruction.
#[test]
fn every_check_is_a_position_of_the_run_at_none() {
    for blob in programs().take(3) {
        let (at_none, ran) = every_check(&blob, (LookAhead::None, Optimise::Speed));
        for how in every_how(&blob.name) {
            let (positions, theirs) = every_check(&blob, how);
            assert_eq!(theirs, ran, "{} at {how:?}", blob.name);
            let mut rest = at_none.iter();
            for position in &positions {
                assert!(
                    rest.any(|p| p == position),
                    "{} at {how:?}: {position:?} is not on the run at none",
                    blob.name
                );
            }
        }
    }
}

/// A stop at a limit, logged at each look-ahead, is reached again exactly from every look-ahead, and the
/// program then ends as it does without stopping.
#[test]
fn stops_are_reached_again_from_every_look_ahead() {
    let seed = setting("WC_PROGRESS_SEED", 1);
    let stops = setting("WC_PROGRESS_STOPS", 1);
    for blob in programs() {
        let plain = run_counting(&blob, (LookAhead::Full, Optimise::Speed));
        let mut rng = generator(seed, b"stops", &blob.name);
        let hows = every_how(&blob.name);
        for &logged_at in &hows {
            for _ in 0..stops {
                let limit = rng.random_range(0..=plain.end);
                let (stop, ran) = first_stop(&blob, logged_at, limit);
                assert_eq!(ran, plain.ran, "{} stopping at {limit}", blob.name);
                let Some(target) = stop else {
                    continue;
                };
                assert!(target.count >= limit);
                for &from in &hows {
                    let (ran, runs) = reach(&blob, from, target);
                    assert_eq!(
                        ran, plain.ran,
                        "{} reaching {target:?} from {from:?}",
                        blob.name
                    );
                    eprintln!(
                        "{}: {target:?} (limit {limit}, logged at {logged_at:?}) reached from {from:?} in {runs} runs",
                        blob.name
                    );
                }
            }
        }
    }
}

/// Not a check: the time of the benchmark set and the leaf call programs at full look-ahead for speed in
/// wasmtime, with the progress count kept against without it, alternating, best of `WC_BENCH_RUNS` (default
/// 3) each; `WC_BENCH_WORKLOADS` picks programs by number from 1 (default all). Run with `--ignored
/// --nocapture` in a release build.
#[test]
#[ignore]
fn measure_progress_count() {
    let runs = setting("WC_BENCH_RUNS", 3);
    let workloads: [(&str, Library, u32); 10] = [
        ("bench", Library::None, 1),
        ("bench", Library::None, 2),
        ("bench", Library::None, 3),
        ("bench", Library::None, 4),
        ("bench", Library::None, 5),
        ("bench", Library::None, 6),
        ("bench", Library::None, 7),
        ("bench", Library::None, 8),
        ("leafcalls", Library::C, 1),
        ("leafcalls", Library::C, 2),
    ];
    let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS")
        .map_or((1..=workloads.len()).collect(), |v| {
            v.split(',').map(|n| n.parse().unwrap()).collect()
        });
    let how = (LookAhead::Full, Optimise::Speed);
    for k in chosen {
        let (name, library, workload) = workloads[k - 1];
        let blob = build_with(
            name,
            "-O2",
            library,
            &["SCALE=1", &format!("WORKLOAD={workload}")],
        );
        let mut best = [f64::MAX; 2];
        let mut ends = [0; 2];
        for round in 0..runs {
            // The order flips each round.
            for side in [round % 2, 1 - round % 2] {
                let mut machine = if side == 0 {
                    plain(&blob, how)
                } else {
                    counting(&blob, how)
                };
                let started = std::time::Instant::now();
                let (ran, end) = finish(&blob, &mut machine);
                best[side as usize] = best[side as usize].min(started.elapsed().as_secs_f64());
                assert_eq!(ran.status, 0, "{name} {workload}");
                ends[side as usize] = end;
            }
        }
        eprintln!(
            "{name} {workload}: without {:.3} s, with the count {:.3} s, {:.3} times; count {} (load {})",
            best[0],
            best[1],
            best[1] / best[0],
            ends[1],
            load()
        );
    }
}

/// A machine with the blob as `how` says and no progress count.
fn plain(blob: &Blob, (look_ahead, optimise): (LookAhead, Optimise)) -> Machine {
    ready_for(blob, look_ahead, optimise, Default::default())
}
