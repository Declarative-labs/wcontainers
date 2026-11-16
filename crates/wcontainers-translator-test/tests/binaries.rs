//! Real ARM64 Linux binaries translated at scale: the code and start addresses of each, what the translator
//! makes of them, and whether the engines take it.

use std::io::Write;
use std::time::Instant;
use wcontainers_translator::LookAhead;
use wcontainers_translator_test::binaries::{Binary, SOURCES};
use wcontainers_translator_test::runner::{Machine, Syscalls, probe};

/// The binaries named in `WC_BINARIES` (comma separated), or all of them.
fn chosen() -> Vec<&'static wcontainers_translator_test::binaries::Source> {
    let names = std::env::var("WC_BINARIES").unwrap_or_default();
    SOURCES
        .iter()
        .filter(|s| names.is_empty() || names.split(',').any(|n| n == s.name))
        .collect()
}

fn gzipped(bytes: &[u8]) -> usize {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap().len()
}

fn validate(wasm: &[u8]) {
    use wasmparser::{Validator, WasmFeatures};
    let features = WasmFeatures::default()
        | WasmFeatures::TAIL_CALL
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::EXCEPTIONS
        | WasmFeatures::THREADS
        | WasmFeatures::MEMORY64
        | WasmFeatures::MULTI_MEMORY
        | WasmFeatures::EXTENDED_CONST
        | WasmFeatures::RELAXED_SIMD
        | WasmFeatures::GC;
    Validator::new_with_features(features)
        .validate_all(wasm)
        .expect("a valid module");
}

/// Not a check: each binary's code and starts, its translation at full look-ahead, and what wasmtime makes
/// of it. One command:
/// `WC_BINARIES=busybox,musl nix develop -c cargo test --release -p wcontainers-translator-test --test binaries measure_binaries -- --ignored --nocapture`
/// (all nine when `WC_BINARIES` is not set; each is taken from its Docker image once).
#[test]
#[ignore]
fn measure_binaries() {
    let host = probe();
    for source in chosen() {
        let file = std::fs::read(source.file()).unwrap();
        let started = Instant::now();
        let binary = Binary::read(source.name, &file);
        let reading = started.elapsed().as_secs_f64();
        let starts = binary.starts.all();
        let code = &binary.image[binary.code.start as usize..binary.code.end as usize];
        let means: Vec<String> = binary
            .starts
            .by_means()
            .iter()
            .map(|(what, set)| format!("{what} {}", set.len()))
            .collect();
        println!(
            "{}: image {} bytes, code {} bytes ({} gzipped, {} instructions), read in {reading:.2} s; {} starts ({})",
            binary.name,
            binary.image.len(),
            code.len(),
            gzipped(code),
            code.len() / 4,
            starts.len(),
            means.join(", ")
        );
        let started = Instant::now();
        let translation = wcontainers_translator::translate(
            &binary.image,
            binary.code.clone(),
            &starts,
            LookAhead::Full,
            host,
        )
        .expect("translate");
        let seconds = started.elapsed().as_secs_f64();
        let pieces: Vec<u64> = translation.pieces().collect();
        let found = pieces
            .iter()
            .filter(|p| starts.binary_search(p).is_err())
            .count();
        let bytes: usize = translation.modules.iter().map(|m| m.wasm.len()).sum();
        let compressed: usize = translation.modules.iter().map(|m| gzipped(&m.wasm)).sum();
        println!(
            "{}: translated in {seconds:.2} s ({:.2} MB of code a second); {} pieces, {found} found by the translator; {} modules, {bytes} bytes ({:.1} a byte of code), {compressed} gzipped ({:.1} a gzipped byte)",
            binary.name,
            code.len() as f64 / seconds / 1e6,
            pieces.len(),
            translation.modules.len(),
            bytes as f64 / code.len() as f64,
            compressed as f64 / gzipped(code) as f64,
        );
        for module in &translation.modules {
            validate(&module.wasm);
        }
        let load = 0x10_0000;
        let memory = (load + binary.image.len() as u64).next_multiple_of(1 << 16);
        let slots = (binary.code.end / 4) as u32;
        let mut machine = Machine::new(memory, load, slots, Syscalls::StandIn);
        let (compiled, instantiated, machine_code) =
            machine.load_translation(&binary.image, binary.code.clone(), load, translation);
        println!(
            "{}: wasmtime compiled it in {:.2} s to {machine_code} bytes of machine code, instantiated it in {:.3} s",
            binary.name,
            compiled.as_secs_f64(),
            instantiated.as_secs_f64(),
        );
    }
}
