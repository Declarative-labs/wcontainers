//! Not a check: times the benchmark set (`programs/bench.c` at full size, -O2) natively and at full
//! look-ahead in wasmtime and Chrome, with and without each check, and prints the size of the translation.
//! Run with `--ignored --nocapture` in a release build. Knobs: `WC_BENCH_RUNS` (best of, default 3),
//! `WC_BENCH_WORKLOADS` (numbers from bench.c, default all), `WC_BENCH_VARIANTS` (which of the five check
//! variants, numbered from 1, default all), `WC_BENCH_CHROME` (0 leaves Chrome out), `WC_BENCH_REPEAT` (how
//! many times Chrome runs the program in the same instances, each run timed by itself, default 1) and
//! `WC_CHROME_JS_FLAGS` (V8 flags for Chrome).

use std::io::Write;
use std::time::{Duration, Instant};
use wcontainers_translator::{LookAhead, Omit, translate};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::programs::{
    Blob, Library, build_with, native, native_seconds, translated_omitting,
};
use wcontainers_translator_test::runner::compile_time;

const WORKLOADS: [&str; 8] = [
    "fib",
    "sieve",
    "hash",
    "sort",
    "interpret",
    "matrix",
    "walk",
    "search",
];

fn variants() -> [(&'static str, Omit); 5] {
    let none = Omit::default();
    [
        ("every check", none),
        ("no stop check", Omit { stop: true, ..none }),
        (
            "no permission check",
            Omit {
                permissions: true,
                ..none
            },
        ),
        (
            "no return check",
            Omit {
                returns: true,
                ..none
            },
        ),
        (
            "no check",
            Omit {
                stop: true,
                permissions: true,
                returns: true,
            },
        ),
    ]
}

fn load() -> String {
    let output = std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .expect("sysctl");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn gzip(bytes: &[u8]) -> usize {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap().len()
}

fn size(blob: &Blob) {
    let code = &blob.bytes[..blob.code_bytes as usize];
    let mut best = (Duration::MAX, Duration::MAX, Vec::new());
    for _ in 0..5 {
        let start = Instant::now();
        let translation =
            translate(&blob.bytes, 0..blob.code_bytes, &[0], LookAhead::Full).unwrap();
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
        "{}: {} bytes of code ({} gzipped); full look-ahead translated in {:?} ({:.1} MB/s); {} bytes of WebAssembly ({:.1} per byte of code), {} gzipped ({:.1} per gzipped byte); wasmtime compile {:?}",
        blob.name,
        code.len(),
        gzip(code),
        best.0,
        code.len() as f64 / best.0.as_secs_f64() / 1e6,
        wasm.len(),
        wasm.len() as f64 / code.len() as f64,
        gzip(wasm),
        gzip(wasm) as f64 / gzip(code) as f64,
        best.1
    );
}

#[test]
#[ignore]
fn measure_benchmarks() {
    let runs: u32 = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
    let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS").map_or((1..=8).collect(), |v| {
        v.split(',').map(|n| n.parse().unwrap()).collect()
    });
    let chrome = std::env::var("WC_BENCH_CHROME").map_or(true, |v| v != "0");
    let repeat: u32 = std::env::var("WC_BENCH_REPEAT").map_or(1, |v| v.parse().unwrap());
    let variants: Vec<(&str, Omit)> = match std::env::var("WC_BENCH_VARIANTS") {
        Ok(v) => v
            .split(',')
            .map(|n| variants()[n.parse::<usize>().unwrap() - 1])
            .collect(),
        Err(_) => variants().to_vec(),
    };
    size(&build_with("bench", "-O2", Library::None, &["SCALE=1"]));
    for k in chosen {
        let name = WORKLOADS[k - 1];
        let workload = format!("WORKLOAD={k}");
        let blob = build_with("bench", "-O2", Library::None, &["SCALE=1", &workload]);
        let expected = native(&blob);
        let native_time = native_seconds(&blob, runs);
        eprintln!("{name}: native {native_time:.3} s (load {})", load());
        for &(label, omit) in &variants {
            let mut best = f64::INFINITY;
            for _ in 0..runs {
                let (ran, _, _, time) = translated_omitting(&blob, LookAhead::Full, omit);
                assert_eq!(ran, expected, "{name} with {label}");
                best = best.min(time.as_secs_f64());
            }
            eprintln!(
                "{name}: wasmtime, {label}: {best:.3} s, {:.2} times native (load {})",
                best / native_time,
                load()
            );
        }
        if !chrome {
            continue;
        }
        for &(label, omit) in &variants {
            // The best time of each run in the same instances.
            let mut best = vec![f64::INFINITY; repeat as usize];
            for _ in 0..runs {
                let report = browser::repeated(&blob, LookAhead::Full, omit, repeat);
                assert_eq!(report.ran, expected, "{name} in Chrome with {label}");
                for (best, time) in best.iter_mut().zip(report.runs) {
                    *best = best.min(time);
                }
            }
            let each: Vec<String> = best
                .iter()
                .map(|t| format!("{t:.3} s ({:.2})", t / native_time))
                .collect();
            eprintln!(
                "{name}: Chrome, {label}: {}, times native in brackets (load {})",
                each.join(", then "),
                load()
            );
        }
    }
}
