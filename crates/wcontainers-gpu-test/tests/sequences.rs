//! Random call sequences, seeded and replayable: each is recorded by the recording back end without
//! breaking a rule, and its call stream replayed on a new one gives the same stream, the same answers
//! and the same contents. `WCONTAINERS_GPU_SEQUENCES` sets how many (default 1000) and
//! `WCONTAINERS_GPU_SEED` the seed (default 0x5eed); the test prints both.

use proptest::test_runner::{Config, RngAlgorithm, RngSeed, TestRunner};
use std::cell::Cell;
use wcontainers_gpu_test::{Recorder, replay, sequence};

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().map_or(default, |v| {
        let v = v.trim();
        match v.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16),
            None => v.parse(),
        }
        .unwrap_or_else(|_| panic!("{name} is not a number: {v}"))
    })
}

#[test]
fn random_sequences_record_and_replay_identically() {
    let cases = knob("WCONTAINERS_GPU_SEQUENCES", 1000);
    let seed = knob("WCONTAINERS_GPU_SEED", 0x5eed);
    let mut runner = TestRunner::new(Config {
        cases: cases as u32,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: None,
        ..Config::default()
    });
    let ran = Cell::new(0u64);
    let calls = Cell::new(0usize);
    let result = runner.run(&sequence::steps(40), |steps| {
        ran.set(ran.get() + 1);
        let mut r = Recorder::new();
        let read = sequence::run(&mut r, &steps).expect("a step was refused");
        assert!(r.violations.is_empty(), "violations: {:?}", r.violations);
        calls.set(calls.get() + r.calls.len());
        let again = replay(&r.calls);
        assert!(
            again.violations.is_empty(),
            "violations: {:?}",
            again.violations
        );
        assert_eq!(again.calls, r.calls);
        assert_eq!(again.contents(), r.contents());
        // The bytes the steps read back are in the stream's finish_read calls.
        let finished: Vec<_> = r
            .calls
            .iter()
            .filter_map(|c| match c {
                wcontainers_gpu_test::Call::FinishRead(_, bytes) => Some(bytes.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(finished, read);
        Ok(())
    });
    eprintln!(
        "random sequences: seed {seed:#x}, {} asked, {} run, {} calls",
        cases,
        ran.get(),
        calls.get()
    );
    result.unwrap();
}
