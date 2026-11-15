//! C programs give the same output and exit status through the translator as on real ARM64 Linux.

use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::programs::{Library, build, native, translated};

const LEVELS: [&str; 5] = ["-O0", "-O1", "-O2", "-O3", "-Os"];

#[test]
fn programs_match_linux() {
    match_linux(&[("integers", Library::None), ("tables", Library::None)]);
}

#[test]
fn blobs_are_told_the_hwcap_of_the_abi() {
    assert_eq!(
        wcontainers_translator_test::layout::BLOB_HWCAP,
        wcontainers_abi::HWCAP
    );
}

#[test]
fn hello_matches_linux() {
    match_linux(&[("hello", Library::C)]);
}

/// Every program, at every optimisation level. Levels that build to the same bytes as an earlier one are
/// run once.
fn match_linux(programs: &[(&str, Library)]) {
    for &(name, library) in programs {
        let mut built: Vec<(&str, Vec<u8>)> = Vec::new();
        for optimisation in LEVELS {
            let blob = build(name, optimisation, library);
            if let Some((same, _)) = built.iter().find(|(_, bytes)| *bytes == blob.bytes) {
                eprintln!("{name} {optimisation}: the same blob as {same}");
                continue;
            }
            built.push((optimisation, blob.bytes.clone()));
            let (ours, machine) = translated(&blob);
            let theirs = native(&blob);
            assert_eq!(
                String::from_utf8_lossy(&ours.stdout),
                String::from_utf8_lossy(&theirs.stdout),
                "output of {name} at {optimisation}"
            );
            assert_eq!(
                ours.stdout, theirs.stdout,
                "output bytes of {name} at {optimisation}"
            );
            assert_eq!(
                String::from_utf8_lossy(&ours.stderr),
                String::from_utf8_lossy(&theirs.stderr),
                "error output of {name} at {optimisation}"
            );
            assert_eq!(
                ours.status, theirs.status,
                "exit status of {name} at {optimisation}"
            );
            eprintln!(
                "{name} {optimisation}: {} bytes of code, matches, {:?}",
                blob.code_bytes,
                machine.counters()
            );
            assert_eq!(machine.count(Reason::UnsupportedInstructionReached), 0);
        }
        eprintln!(
            "{name}: {} distinct blobs from {} levels",
            built.len(),
            LEVELS.len()
        );
    }
}

/// Not a check: prints translation speed, output size raw and gzip-compressed, and compile time for every
/// instruction of each distinct program at look-ahead none, best of five runs.
#[test]
fn measure() {
    use std::io::Write;
    use std::time::{Duration, Instant};
    use wcontainers_translator::{LookAhead, translate};
    use wcontainers_translator_test::runner::compile_time;
    let gzip = |bytes: &[u8]| {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap().len()
    };
    for (name, optimisation) in [
        ("integers", "-O0"),
        ("integers", "-O2"),
        ("tables", "-O0"),
        ("tables", "-O2"),
    ] {
        let blob = build(name, optimisation, Library::None);
        let code = &blob.bytes[..blob.code_bytes as usize];
        let starts: Vec<u64> = (0..blob.code_bytes / 4).map(|i| 4 * i).collect();
        let mut best = (Duration::MAX, Duration::MAX, Vec::new());
        for _ in 0..5 {
            let start = Instant::now();
            let translation =
                translate(&blob.bytes, 0..blob.code_bytes, &starts, LookAhead::None).unwrap();
            let translating = start.elapsed();
            let compiling = compile_time(&translation.wasm);
            best = (
                best.0.min(translating),
                best.1.min(compiling),
                translation.wasm,
            );
        }
        let wasm = &best.2;
        eprintln!(
            "{name} {optimisation}: {} bytes of code ({} gzipped), {} instructions; translated in {:?} ({:.1} MB/s); {} bytes of WebAssembly ({:.1} per byte of code), {} gzipped ({:.1} per gzipped byte of code); compiled by wasmtime in {:?}",
            code.len(),
            gzip(code),
            starts.len(),
            best.0,
            code.len() as f64 / best.0.as_secs_f64() / 1e6,
            wasm.len(),
            wasm.len() as f64 / code.len() as f64,
            gzip(wasm),
            gzip(wasm) as f64 / gzip(code) as f64,
            best.1
        );
    }
}
