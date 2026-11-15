//! Layer 2: random small programs give the same result at full look-ahead as at none, register by register
//! and byte by byte in the same engine, and the same output as on the real CPU.
//!
//! Knobs: `WC_LAYER2_CASES` (default 64), `WC_LAYER2_SEED` (default 1), `WC_LAYER2_NATIVE` (how many of the
//! cases also run on the real CPU, default 8; each starts a container).

use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::layer2::{OUTPUT, assemble, program};
use wcontainers_translator_test::programs::{native, reachable_memory, translated};

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

#[test]
fn full_matches_none_and_the_cpu() {
    let cases = knob("WC_LAYER2_CASES", 64);
    let seed = knob("WC_LAYER2_SEED", 1);
    let natives = knob("WC_LAYER2_NATIVE", 8);
    let mut seed_bytes = [0u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: cases as u32,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed_bytes),
    );
    let ran = std::cell::Cell::new(0u64);
    let result = runner.run(&program(), |program| {
        let blob = assemble(&program);
        let (none, mut none_machine, none_registers) = translated(&blob, LookAhead::None);
        let (full, mut full_machine, full_registers) = translated(&blob, LookAhead::Full);
        assert_eq!(none.status, 0);
        assert_eq!(none.stdout.len(), OUTPUT as usize);
        assert_eq!(full, none, "output at full against none");
        assert_eq!(
            full_registers, none_registers,
            "registers at full against none"
        );
        assert!(
            reachable_memory(&mut full_machine, &blob)
                == reachable_memory(&mut none_machine, &blob),
            "memory at full against none"
        );
        assert_eq!(full_machine.count(Reason::UnsupportedInstructionReached), 0);
        if ran.get() < natives {
            assert_eq!(native(&blob), none, "output against the CPU");
        }
        ran.set(ran.get() + 1);
        Ok(())
    });
    eprintln!("layer 2: {} cases run, seed {seed}", ran.get());
    if let Err(error) = result {
        panic!("{error}");
    }
}
