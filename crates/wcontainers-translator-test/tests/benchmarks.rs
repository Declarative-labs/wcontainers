//! Not a check: times the benchmark set (`programs/bench.c` at full size, -O2) natively and at full
//! look-ahead in wasmtime and Chrome, with and without each check, and prints the size of the translation.
//! Run with `--ignored --nocapture` in a release build. Knobs: `WC_BENCH_RUNS` (best of, default 3),
//! `WC_BENCH_WORKLOADS` (numbers from bench.c, default all), `WC_BENCH_VARIANTS` (which of the six check
//! variants, numbered from 1, default all), `WC_BENCH_CHROME` (0 leaves Chrome out), `WC_BENCH_REPEAT` (how
//! many times Chrome runs the program in the same instances, each run timed by itself, default 1),
//! `WC_CHROME_JS_FLAGS` (V8 flags for Chrome) and `WC_OPTIMISE` (`size` translates for size, default speed).

use std::io::Write;
use std::time::{Duration, Instant};
use wcontainers_translator::{LookAhead, Omit, Optimise, translate};
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

fn variants() -> [(&'static str, Omit); 6] {
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
            "no depth check",
            Omit {
                depth: true,
                ..none
            },
        ),
        (
            "no check",
            Omit {
                stop: true,
                permissions: true,
                returns: true,
                depth: true,
            },
        ),
    ]
}

/// Where Linux gives the load averages; macOS has none, and `sysctl` gives them there.
const LOAD_AVERAGES: &str = "/proc/loadavg";

fn load() -> String {
    if let Ok(averages) = std::fs::read_to_string(LOAD_AVERAGES) {
        return averages.trim().to_string();
    }
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

/// What the translations are made for, from `WC_OPTIMISE`.
fn optimise() -> Optimise {
    match std::env::var("WC_OPTIMISE").as_deref() {
        Ok("size") => Optimise::Size,
        _ => Optimise::Speed,
    }
}

fn size(blob: &Blob) {
    let code = &blob.bytes[..blob.code_bytes as usize];
    let mut best = (Duration::MAX, Duration::MAX, Vec::new());
    for _ in 0..5 {
        let start = Instant::now();
        let mut translation = translate::<wcontainers_arm64::Arm64>(
            &blob.bytes,
            0..blob.code_bytes,
            &[0],
            LookAhead::Full,
            optimise(),
            wcontainers_translator_test::runner::probe(),
            (),
        )
        .unwrap();
        let translating = start.elapsed();
        let compiling = compile_time(&translation.modules[0].wasm);
        best = (
            best.0.min(translating),
            best.1.min(compiling),
            translation.modules.remove(0).wasm,
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
    eprintln!("translated for {:?}", optimise());
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
            let mut counters = Vec::new();
            for _ in 0..runs {
                let (ran, machine, _, time) =
                    translated_omitting(&blob, (LookAhead::Full, optimise()), omit);
                assert_eq!(ran, expected, "{name} with {label}");
                best = best.min(time.as_secs_f64());
                counters = machine.counters();
            }
            eprintln!("{name}: wasmtime, {label}: counters {counters:?}");
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
                let report = browser::repeated(&blob, (LookAhead::Full, optimise()), omit, repeat);
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

/// Not a check: what a direct call costs when it crosses from one code module to another, in wasmtime and
/// Chrome (`programs/calls.c`, -O2, `WC_CALLS` calls of a small function, default 100 million): the program
/// in one module, and with every piece in a module of its own, so that each call goes through the lookup.
/// Best of `WC_BENCH_RUNS`, for both settings; Chrome runs the program twice in the same instances, and
/// `WC_BENCH_CHROME=0` leaves it out.
#[test]
#[ignore]
fn measure_calls_between_modules() {
    use wcontainers_translator_test::programs::{ran, ready_with};
    let runs: u32 = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
    let calls: u64 = std::env::var("WC_CALLS").map_or(100_000_000, |v| v.parse().unwrap());
    let define = format!("CALLS={calls}");
    let blob = build_with("calls", "-O2", Library::None, &[&define]);
    let expected = native(&blob);
    let native_time = native_seconds(&blob, runs);
    eprintln!("calls: native {native_time:.3} s (load {})", load());
    for optimise in [Optimise::Speed, Optimise::Size] {
        let mut one = f64::INFINITY;
        let mut split = f64::INFINITY;
        for _ in 0..runs {
            let (ran_one, _, _, time) =
                translated_omitting(&blob, (LookAhead::Full, optimise), Omit::default());
            assert_eq!(ran_one, expected);
            one = one.min(time.as_secs_f64());
            let mut machine =
                ready_with(&blob, wcontainers_translator_test::runner::probe(), |m| {
                    m.optimise_for(optimise);
                    m.load_split(&blob.bytes, 0..blob.code_bytes, blob.load, &[0], 1);
                });
            let started = Instant::now();
            let (stopped, registers) = machine.run();
            let time = started.elapsed().as_secs_f64();
            assert_eq!(ran(&blob, &machine, stopped, &registers), expected);
            split = split.min(time);
        }
        eprintln!(
            "calls for {optimise:?}: wasmtime, one module {one:.3} s, a module a piece {split:.3} s: {:.2} ns more a call (load {})",
            (split - one) / calls as f64 * 1e9,
            load()
        );
        if std::env::var("WC_BENCH_CHROME").is_ok_and(|v| v == "0") {
            continue;
        }
        let (mut one, mut split) = (f64::INFINITY, f64::INFINITY);
        for _ in 0..runs {
            let report = browser::repeated(&blob, (LookAhead::Full, optimise), Omit::default(), 2);
            assert_eq!(report.ran, expected);
            one = report.runs.iter().fold(one, |a, &b| a.min(b));
            let report = browser::repeated_split(&blob, optimise, 2, 1);
            assert_eq!(report.ran, expected);
            split = report.runs.iter().fold(split, |a, &b| a.min(b));
        }
        eprintln!(
            "calls for {optimise:?}: Chrome, one module {one:.3} s, a module a piece {split:.3} s: {:.2} ns more a call (load {})",
            (split - one) / calls as f64 * 1e9,
            load()
        );
    }
}
