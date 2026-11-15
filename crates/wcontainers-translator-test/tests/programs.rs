//! Small C programs give the same output and exit status through the translator as on real ARM64 Linux.

use wcontainers_translator::contract::Reason;
use wcontainers_translator_test::programs::{build, native, translated};

#[test]
fn integers() {
    for optimisation in ["-O0", "-O1", "-O2"] {
        let blob = build("integers", optimisation);
        let (ours, machine) = translated(&blob);
        let theirs = native(&blob);
        assert_eq!(
            String::from_utf8_lossy(&ours.output),
            String::from_utf8_lossy(&theirs.output),
            "output at {optimisation}"
        );
        assert_eq!(ours.status, theirs.status, "exit status at {optimisation}");
        let counters = machine.counters();
        eprintln!(
            "integers {optimisation}: {} bytes of code, {:?}",
            blob.code_bytes, counters
        );
        assert_eq!(machine.count(Reason::UnsupportedInstructionReached), 0);
    }
}

/// Not a check: prints translation speed, output size and compile time for every instruction of each
/// program at look-ahead none, best of five runs.
#[test]
fn measure() {
    use std::time::{Duration, Instant};
    use wcontainers_translator::{LookAhead, translate};
    use wcontainers_translator_test::runner::compile_time;
    for optimisation in ["-O0", "-O2"] {
        let blob = build("integers", optimisation);
        let starts: Vec<u64> = (0..blob.code_bytes / 4).map(|i| 4 * i).collect();
        let mut best = (Duration::MAX, Duration::MAX, 0);
        for _ in 0..5 {
            let start = Instant::now();
            let translation =
                translate(&blob.bytes, 0..blob.code_bytes, &starts, LookAhead::None).unwrap();
            let translating = start.elapsed();
            let compiling = compile_time(&translation.wasm);
            best = (
                best.0.min(translating),
                best.1.min(compiling),
                translation.wasm.len(),
            );
        }
        eprintln!(
            "integers {optimisation}: {} bytes of code, {} instructions; translated in {:?} ({:.1} MB/s); {} bytes of WebAssembly ({:.1} per byte of code); compiled by wasmtime in {:?}",
            blob.code_bytes,
            starts.len(),
            best.0,
            blob.code_bytes as f64 / best.0.as_secs_f64() / 1e6,
            best.2,
            best.2 as f64 / blob.code_bytes as f64,
            best.1
        );
    }
}
