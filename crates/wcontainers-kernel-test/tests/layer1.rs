//! Layer 1: random sequences of system calls against the reference, comparing every value and every byte
//! written back. `LAYER1_CASES` sets how many sequences run (default 64) and `LAYER1_SEED` the seed
//! (default 0); a failure is shrunk and printed with its seed.

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
    let cases = knob("LAYER1_CASES", 64) as u32;
    let seed = knob("LAYER1_SEED", 0);
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let runner = layer1::runner(&out);
    let program = std::fs::read(&runner).expect("reading the runner");
    let reference = std::cell::RefCell::new(layer1::Reference::start(&runner));
    let mut seed_bytes = [0u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let config = Config {
        cases,
        failure_persistence: None,
        max_shrink_iters: 2000,
        // A case that waits costs the reference's 60 s deadline at each step.
        max_shrink_time: 30 * 60 * 1000,
        ..Config::default()
    };
    let mut runner = TestRunner::new_with_rng(
        config,
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed_bytes),
    );
    let strategy = (
        proptest::collection::vec(any::<u64>(), 125),
        proptest::collection::vec((any::<u8>(), any::<[u64; 6]>()), 1..16),
    );
    // How many calls put a random device's bytes somewhere, to show they are reached.
    let random = std::cell::Cell::new(0u64);
    let result = runner.run(&strategy, |(values, raw)| {
        let sequence = layer1::build(&values, &raw);
        let theirs = reference.borrow_mut().run(&sequence);
        random.set(random.get() + theirs.calls.iter().filter(|call| call.random).count() as u64);
        let ours = layer1::run_ours(&sequence, &program, &theirs);
        match layer1::difference(&sequence, &ours, &theirs) {
            None => Ok(()),
            Some(text) => Err(TestCaseError::fail(text)),
        }
    });
    eprintln!(
        "layer 1: {cases} sequences, seed {seed}, {} calls with a random device's bytes",
        random.get()
    );
    if let Err(error) = result {
        panic!("layer 1 (seed {seed}): {error}");
    }
}
