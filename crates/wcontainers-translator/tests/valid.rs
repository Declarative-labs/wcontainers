//! Every module the translator makes is valid WebAssembly. Cases: `PROPTEST_CASES` (default 256); seed:
//! `PROPTEST_RNG_SEED` (default 1).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use wasmparser::{Validator, WasmFeatures};
use wcontainers_translator::{LookAhead, thread_module, translate};

fn validate(wasm: &[u8]) {
    let features = WasmFeatures::default()
        | WasmFeatures::TAIL_CALL
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::EXCEPTIONS
        | WasmFeatures::THREADS
        | WasmFeatures::MEMORY64
        | WasmFeatures::MULTI_MEMORY
        | WasmFeatures::EXTENDED_CONST;
    if let Err(error) = Validator::new_with_features(features).validate_all(wasm) {
        panic!("invalid module: {error}");
    }
}

#[test]
fn thread_module_is_valid() {
    validate(&thread_module(1024));
}

/// The environment's settings, with a fixed seed when none is given.
fn config() -> Config {
    let mut config = Config::default();
    if config.rng_seed == RngSeed::Random {
        config.rng_seed = RngSeed::Fixed(1);
    }
    config
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn code_modules_are_valid(words in prop::collection::vec(any::<u32>(), 1..64)) {
        let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let starts: Vec<u64> = (0..words.len() as u64).map(|i| 4 * i).collect();
        let translation = translate(&blob, 0..blob.len() as u64, &starts, LookAhead::None).unwrap();
        validate(&translation.wasm);
    }
}
