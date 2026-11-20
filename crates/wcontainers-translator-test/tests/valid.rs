//! Every module the translator makes of random code is valid WebAssembly, for each guest. Cases:
//! `PROPTEST_CASES` (default 256); seed: `PROPTEST_RNG_SEED` (default 1).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use wasmparser::{Validator, WasmFeatures};
use wcontainers_arm64::{Arm64, Host};
use wcontainers_translator::{LookAhead, Optimise, thread_module, translate};
use wcontainers_x86::{Offer, X86_64};

/// A host that needs every fix-up and the software fused multiply-add, so their code is checked too.
const HOST: Host = Host {
    fused_multiply_add: false,
    arm_nans: false,
};

/// A host whose NaNs are not x86's, so the fix-ups' code is checked too.
const HOST_X86_64: wcontainers_x86::Host = wcontainers_x86::Host { x86_nans: false };

fn validate(wasm: &[u8]) {
    if let Err(error) = check(wasm) {
        panic!("invalid module: {error}");
    }
}

fn check(wasm: &[u8]) -> Result<(), String> {
    let features = WasmFeatures::default()
        | WasmFeatures::TAIL_CALL
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::EXCEPTIONS
        | WasmFeatures::THREADS
        | WasmFeatures::MEMORY64
        | WasmFeatures::MULTI_MEMORY
        | WasmFeatures::EXTENDED_CONST
        | WasmFeatures::RELAXED_SIMD;
    Validator::new_with_features(features)
        .validate_all(wasm)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn thread_module_is_valid_arm64() {
    validate(&thread_module::<Arm64>());
}

#[test]
fn thread_module_is_valid_x86_64() {
    validate(&thread_module::<X86_64>());
}

/// The most random bytes an x86 case translates, 255: many instructions of every length.
const CASE_BYTES: usize = u8::MAX as usize;

/// The starts (sorted) where a whole x86 instruction starts within the bytes.
fn whole_x86_64(bytes: &[u8], starts: &[u64]) -> Vec<u64> {
    use wcontainers_guest_cpu::Guest;
    starts
        .iter()
        .copied()
        .filter(|&s| X86_64::decode(&bytes[s as usize..]).is_some())
        .collect()
}

/// The environment's settings, with a fixed seed when none is given.
fn config() -> Config {
    let mut config = Config::default();
    if config.rng_seed == RngSeed::Random {
        config.rng_seed = RngSeed::Fixed(1);
    }
    config
}

/// A branch whose target wraps below the blob's start.
#[test]
fn target_below_zero_arm64() {
    // tbz x0, #0, -32768
    let blob = 0x3604_0000u32.to_le_bytes();
    let translation = translate::<Arm64>(
        &blob,
        0..4,
        &[0],
        LookAhead::None,
        Optimise::Speed,
        HOST,
        (),
    )
    .unwrap();
    assert_eq!(translation.modules[0].pieces, [0]);
    validate(&translation.modules[0].wasm);
}

/// A chain of thousands of branches, each block nested in the next one's merge, with more blocks than one
/// piece may hold: translated at full on a small stack, cut in parts, and valid.
#[test]
fn long_chains_translate_on_a_small_stack_arm64() {
    let mut words = Vec::new();
    for _ in 0..4000 {
        words.push(0xb400_0041u32); // cbz x1, +8
        words.push(0x9100_0400); // add x0, x0, #1
    }
    words.push(0xd65f_03c0); // ret
    let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let translation = std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || {
            translate::<Arm64>(
                &blob,
                0..blob.len() as u64,
                &[0],
                LookAhead::Full,
                Optimise::Speed,
                HOST,
                (),
            )
        })
        .unwrap()
        .join()
        .unwrap()
        .unwrap();
    assert!(translation.pieces().count() > 1);
    for module in &translation.modules {
        validate(&module.wasm);
    }
}

proptest! {
    #![proptest_config(config())]
    #[test]
    fn code_modules_are_valid_arm64(words in prop::collection::vec(any::<u32>(), 1..64)) {
        let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let starts: Vec<u64> = (0..words.len() as u64).map(|i| 4 * i).collect();
        let translation = translate::<Arm64>(&blob, 0..blob.len() as u64, &starts, LookAhead::None, Optimise::Speed, HOST, ()).unwrap();
        validate(&translation.modules[0].wasm);
    }

    /// Random words at full look-ahead, from the first word and from a few others.
    #[test]
    fn full_modules_are_valid_arm64(words in prop::collection::vec(any::<u32>(), 1..64), picks in prop::collection::vec(any::<prop::sample::Index>(), 0..4)) {
        let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut starts = vec![0];
        starts.extend(picks.iter().map(|p| 4 * p.index(words.len()) as u64));
        let translation = translate::<Arm64>(&blob, 0..blob.len() as u64, &starts, LookAhead::Full, Optimise::Speed, HOST, ()).unwrap();
        validate(&translation.modules[0].wasm);
    }

    /// Random bytes translated from every byte address at look-ahead none: x86 code may start anywhere. A
    /// piece for every start but those whose instruction runs past the end of the bytes.
    #[test]
    fn code_modules_are_valid_x86_64(bytes in prop::collection::vec(any::<u8>(), 1..=CASE_BYTES)) {
        let starts: Vec<u64> = (0..bytes.len() as u64).collect();
        let translation = translate::<X86_64>(&bytes, 0..bytes.len() as u64, &starts, LookAhead::None, Optimise::Speed, HOST_X86_64, Offer::default()).unwrap();
        prop_assert_eq!(&translation.modules[0].pieces, &whole_x86_64(&bytes, &starts));
        validate(&translation.modules[0].wasm);
    }

    /// Random bytes at full look-ahead, from the first byte and from a few others.
    #[test]
    fn full_modules_are_valid_x86_64(bytes in prop::collection::vec(any::<u8>(), 1..=CASE_BYTES), picks in prop::collection::vec(any::<prop::sample::Index>(), 0..4)) {
        let mut starts = vec![0];
        starts.extend(picks.iter().map(|p| p.index(bytes.len()) as u64));
        let translation = translate::<X86_64>(&bytes, 0..bytes.len() as u64, &starts, LookAhead::Full, Optimise::Speed, HOST_X86_64, Offer::default()).unwrap();
        for module in &translation.modules {
            validate(&module.wasm);
        }
    }

    /// Random bytes at bounded look-ahead of random N, from a few byte addresses: one piece in the table each.
    #[test]
    fn bounded_modules_are_valid_x86_64(bytes in prop::collection::vec(any::<u8>(), 1..=CASE_BYTES), picks in prop::collection::vec(any::<prop::sample::Index>(), 1..4), bound in 1..80u32) {
        let mut starts: Vec<u64> = picks.iter().map(|p| p.index(bytes.len()) as u64).collect();
        let translation = translate::<X86_64>(&bytes, 0..bytes.len() as u64, &starts, LookAhead::Bounded(bound), Optimise::Speed, HOST_X86_64, Offer::default()).unwrap();
        starts.sort();
        starts.dedup();
        prop_assert_eq!(&translation.modules[0].pieces, &whole_x86_64(&bytes, &starts));
        validate(&translation.modules[0].wasm);
    }

    /// Random words at bounded look-ahead of random N, from a few starts: one piece in the table each.
    #[test]
    fn bounded_modules_are_valid_arm64(words in prop::collection::vec(any::<u32>(), 1..64), picks in prop::collection::vec(any::<prop::sample::Index>(), 1..4), bound in 1..80u32) {
        let blob: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut starts: Vec<u64> = picks.iter().map(|p| 4 * p.index(words.len()) as u64).collect();
        let translation = translate::<Arm64>(&blob, 0..blob.len() as u64, &starts, LookAhead::Bounded(bound), Optimise::Speed, HOST, ()).unwrap();
        starts.sort();
        starts.dedup();
        prop_assert_eq!(&translation.modules[0].pieces, &starts);
        validate(&translation.modules[0].wasm);
    }
}

/// Words of every form of the CPU we present, each translated alone at look-ahead none and with others
/// at full, on a host that needs every fix-up: valid modules. `WC_VALID_WORDS` words a form (default 16).
#[test]
fn every_form_is_valid_arm64() {
    use proptest::test_runner::{RngAlgorithm, TestRng};
    let count: u32 = std::env::var("WC_VALID_WORDS").map_or(16, |v| v.parse().unwrap());
    let mut rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[7; 32]);
    let mut words = Vec::new();
    for form in wcontainers_arm64::forms::all() {
        for _ in 0..count {
            let word = form.word(rng.random());
            if form.presented(word) {
                words.push(word);
            }
        }
    }
    let mut failures = Vec::new();
    std::panic::set_hook(Box::new(|_| {}));
    for &word in &words {
        let translated = std::panic::catch_unwind(|| {
            translate::<Arm64>(
                &word.to_le_bytes(),
                0..4,
                &[0],
                LookAhead::None,
                Optimise::Speed,
                HOST,
                (),
            )
            .unwrap()
        });
        let translation = match translated {
            Ok(t) => t,
            Err(panic) => {
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                failures.push(format!("{word:#010x}: panicked: {message}"));
                continue;
            }
        };
        if let Err(error) = check(&translation.modules[0].wasm) {
            failures.push(format!(
                "{word:#010x} {:?}: {error}",
                wcontainers_arm64::decode(word)
            ));
        }
    }
    let _ = std::panic::take_hook();
    failures.sort();
    failures.dedup_by(|a, b| a.split(':').nth(1) == b.split(':').nth(1));
    assert!(
        failures.is_empty(),
        "{} invalid:\n{}",
        failures.len(),
        failures.join("\n")
    );
    for chunk in words.chunks(64) {
        let blob: Vec<u8> = chunk.iter().flat_map(|w| w.to_le_bytes()).collect();
        let starts: Vec<u64> = (0..chunk.len() as u64).map(|i| 4 * i).collect();
        for look_ahead in [LookAhead::None, LookAhead::Bounded(16), LookAhead::Full] {
            let translation = translate::<Arm64>(
                &blob,
                0..blob.len() as u64,
                &starts,
                look_ahead,
                Optimise::Speed,
                HOST,
                (),
            )
            .unwrap();
            validate(&translation.modules[0].wasm);
        }
    }
}
