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

/// The settings named in `WC_OPTIMISE` (`speed` or `size`), or both.
fn settings() -> Vec<wcontainers_translator::Optimise> {
    use wcontainers_translator::Optimise;
    match std::env::var("WC_OPTIMISE").as_deref() {
        Ok("speed") => vec![Optimise::Speed],
        Ok("size") => vec![Optimise::Size],
        _ => vec![Optimise::Speed, Optimise::Size],
    }
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
/// (all nine when `WC_BINARIES` is not set; each is taken from its Docker image once). `WC_BINARIES_DUMP`
/// names a directory to write each module to, for looking into; `WC_BINARIES_ENGINES=0` leaves the engines
/// out; `WC_OPTIMISE` (`speed` or `size`) translates for one setting only, not both.
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
        for optimise in settings() {
            let name = format!("{} for {optimise:?}", binary.name);
            let started = Instant::now();
            let translation = wcontainers_translator::translate(
                &binary.image,
                binary.code.clone(),
                &starts,
                LookAhead::Full,
                optimise,
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
                name,
                code.len() as f64 / seconds / 1e6,
                pieces.len(),
                translation.modules.len(),
                bytes as f64 / code.len() as f64,
                compressed as f64 / gzipped(code) as f64,
            );
            for (i, module) in translation.modules.iter().enumerate() {
                validate(&module.wasm);
                if let Ok(directory) = std::env::var("WC_BINARIES_DUMP") {
                    let path = format!("{directory}/{}-{optimise:?}-{i}.wasm", binary.name);
                    std::fs::write(path, &module.wasm).unwrap();
                }
            }
            if std::env::var("WC_BINARIES_ENGINES").is_ok_and(|v| v == "0") {
                continue;
            }
            let load = 0x10_0000;
            let wasm: Vec<&[u8]> = translation.modules.iter().map(|m| &m.wasm[..]).collect();
            for (what, flags) in [
                ("compiled at first call", ""),
                ("compiled at once", "--no-wasm-lazy-compilation"),
            ] {
                match wcontainers_translator_test::browser::load_modules(
                    &name,
                    &binary.image,
                    load,
                    binary.code.clone(),
                    &wasm,
                    flags,
                ) {
                    Ok((compiling, instantiating)) => println!(
                        "{}: Chrome, {what}, made its modules in {:.2} s and their instances in {:.3} s",
                        name,
                        compiling / 1e3,
                        instantiating / 1e3
                    ),
                    Err(error) => println!("{}: Chrome, {what}, refused: {error}", binary.name),
                }
            }
            let memory = (load + binary.image.len() as u64).next_multiple_of(1 << 16);
            // The lookup table covers the code alone.
            let slots = ((binary.code.end - binary.code.start) / 4) as u32;
            let table_base = load + binary.code.start;
            let mut machine = Machine::new(memory, table_base, slots, Syscalls::StandIn);
            let (compiled, instantiated, machine_code) =
                machine.load_translation(&binary.image, binary.code.clone(), load, translation);
            println!(
                "{}: wasmtime compiled it in {:.2} s to {machine_code} bytes of machine code, instantiated it in {:.3} s",
                name,
                compiled.as_secs_f64(),
                instantiated.as_secs_f64(),
            );
        }
    }
}
