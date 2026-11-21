//! Layer 1: random sequences of system calls against the reference, comparing every value and every byte
//! written back. `LAYER1_CASES` sets how many sequences run (default 64) and `LAYER1_SEED` the seed
//! (default 0); a failure is shrunk and printed with its seed. On the file backend,
//! `LAYER1_BACKEND_CASES` sets how many (default 32), and on a backend that opens files for the
//! page cache `LAYER1_CACHED_CASES` (default 32), with the same seed. `LAYER1_REPLAY_CASES` (default
//! 64) sets how many of each kind run on our kernel alone, then again from their record alone.

use std::path::Path;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use wcontainers_kernel_test::layer1;

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

#[test]
fn sequences_match_reference() {
    sequences(knob("LAYER1_CASES", 64) as u32, false, false);
}

/// Sequences of file calls whose work directory is on the file backend, against the reference
/// with the same server mounted through Linux's FUSE.
#[test]
fn sequences_on_backend_match_reference() {
    sequences(knob("LAYER1_BACKEND_CASES", 32) as u32, true, false);
}

/// The same with the server opening files for the page cache, as Linux's FUSE then caches them.
#[test]
fn sequences_on_cached_backend_match_reference() {
    sequences(knob("LAYER1_CACHED_CASES", 32) as u32, true, true);
}

/// Sequences in memory, on the backend and on the cached backend, each run on our kernel keeping a
/// record of what the host gave it, then on a fresh kernel from that record alone: the same values
/// and the same guest memory.
#[cfg(not(feature = "runtime"))]
#[test]
fn sequences_replay_from_their_record() {
    let cases = knob("LAYER1_REPLAY_CASES", 64) as u32;
    let seed = knob("LAYER1_SEED", 0);
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = std::fs::read(layer1::runner(&out)).expect("reading the runner");
    let server = std::cell::RefCell::new(None);
    // No reference: nothing of the runner's is cleared after a call.
    let none = layer1::SequenceResult {
        calls: Vec::new(),
        status: 0,
        play: None,
    };
    for (backend, cached) in [(false, false), (true, false), (true, true)] {
        let entries = std::cell::Cell::new(0);
        let result = case_runner(cases, seed).run(&strategy(), |(values, raw)| {
            let sequence = if backend {
                layer1::build_on_backend(&values, &raw, cached)
            } else {
                layer1::build(&values, &raw)
            };
            let (_, kept) =
                layer1::run_ours_replayed(&sequence, &program, &none, &mut server.borrow_mut());
            entries.set(entries.get() + kept);
            Ok(())
        });
        eprintln!(
            "layer 1 replayed{}: {cases} sequences, seed {seed}, {} entries in their records",
            if cached {
                " on the cached backend"
            } else if backend {
                " on the backend"
            } else {
                ""
            },
            entries.get()
        );
        if let Err(error) = result {
            panic!("layer 1 replayed (seed {seed}): {error}");
        }
    }
}

/// What a sequence is made from: the arena's values and the calls.
fn strategy() -> impl Strategy<Value = (Vec<u64>, Vec<(u16, [u64; 6])>)> {
    (
        proptest::collection::vec(any::<u64>(), 125),
        proptest::collection::vec((any::<u16>(), any::<[u64; 6]>()), 1..16),
    )
}

/// A runner of `cases` sequences drawn from `seed`.
fn case_runner(cases: u32, seed: u64) -> TestRunner {
    let mut seed_bytes = [0u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let config = Config {
        cases,
        failure_persistence: None,
        max_shrink_iters: 2000,
        // A case that waits costs the reference's 60 s deadline at each step.
        max_shrink_time: u32::try_from(std::time::Duration::from_secs(30 * 60).as_millis())
            .expect("half an hour in milliseconds"),
        ..Config::default()
    };
    TestRunner::new_with_rng(
        config,
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed_bytes),
    )
}

fn sequences(cases: u32, backend: bool, cached: bool) {
    let seed = knob("LAYER1_SEED", 0);
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let runner = layer1::runner(&out);
    let program = std::fs::read(&runner).expect("reading the runner");
    let reference = std::cell::RefCell::new(layer1::Reference::start(&runner, backend, cached));
    let server = std::cell::RefCell::new(None);
    let mut runner = case_runner(cases, seed);
    let strategy = strategy();
    // How many calls put a random device's bytes somewhere, to show they are reached.
    let random = std::cell::Cell::new(0u64);
    // And how many made or opened the named pipe.
    let named_pipe = std::cell::Cell::new(0u64);
    // And how many opened a pseudo-terminal.
    let terminal = std::cell::Cell::new(0u64);
    // And how many children were made.
    let children = std::cell::Cell::new(0u64);
    // And how many sends with control messages and receives went well.
    let messages = std::cell::Cell::new((0u64, 0u64));
    let result = runner.run(&strategy, |(values, raw)| {
        let sequence = if backend {
            layer1::build_on_backend(&values, &raw, cached)
        } else {
            layer1::build(&values, &raw)
        };
        let theirs = reference.borrow_mut().run(&sequence);
        random.set(random.get() + theirs.calls.iter().filter(|call| call.random).count() as u64);
        named_pipe.set(named_pipe.get() + layer1::named_pipe_calls(&sequence, &theirs));
        terminal.set(terminal.get() + layer1::terminal_opens(&sequence, &theirs));
        children.set(children.get() + layer1::children_made(&sequence, &theirs));
        let (sends, receives) = layer1::message_calls(&sequence, &theirs);
        let (all_sends, all_receives) = messages.get();
        messages.set((all_sends + sends, all_receives + receives));
        let ours = layer1::run_ours_with(&sequence, &program, &theirs, &mut server.borrow_mut());
        match layer1::difference(&sequence, &ours, &theirs) {
            None => Ok(()),
            Some(text) => Err(TestCaseError::fail(text)),
        }
    });
    eprintln!(
        "layer 1{}: {cases} sequences, seed {seed}, {} calls with a random device's bytes, {} that \
         made or opened the named pipe, {} that opened a pseudo-terminal, {} children made, {} sends \
         with control messages and {} receives that went well",
        if backend { " on the backend" } else { "" },
        random.get(),
        named_pipe.get(),
        terminal.get(),
        children.get(),
        messages.get().0,
        messages.get().1
    );
    if let Err(error) = result {
        panic!("layer 1 (seed {seed}): {error}");
    }
}
