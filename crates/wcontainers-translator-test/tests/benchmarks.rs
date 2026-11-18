//! Not a check: times the benchmark set (`programs/bench.c` at full size, -O2) natively and at full
//! look-ahead in wasmtime and Chrome, with and without each check, and prints the size of the translation.
//! Each is written once over the guest and run for each (`measure_benchmarks_arm64`). Run with `--ignored
//! --nocapture` in a release build. Knobs: `WC_BENCH_RUNS` (best of, default 3),
//! `WC_BENCH_WORKLOADS` (numbers from bench.c, default all), `WC_BENCH_VARIANTS` (which of the six check
//! variants, numbered from 1, default all), `WC_BENCH_CHROME` (0 leaves Chrome out), `WC_BENCH_REPEAT` (how
//! many times Chrome runs the program in the same instances, each run timed by itself, default 1),
//! `WC_CHROME_JS_FLAGS` (V8 flags for Chrome) and `WC_OPTIMISE` (`size` translates for size, default speed).

use std::io::Write;
use std::time::{Duration, Instant};
use wcontainers_arm64::Arm64;
use wcontainers_translator::{LookAhead, Omit, Optimise, translate};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{
    Blob, Library, build_with, native, native_seconds, translated_omitting,
};
use wcontainers_translator_test::runner::compile_time;
use wcontainers_x86::X86_64;

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

/// What the translations are made for, from `WC_OPTIMISE`.
fn optimise() -> Optimise {
    match std::env::var("WC_OPTIMISE").as_deref() {
        Ok("size") => Optimise::Size,
        _ => Optimise::Speed,
    }
}

fn size<G: GuestTest>(blob: &Blob) {
    let code = &blob.bytes[..blob.code_bytes as usize];
    let mut best = (Duration::MAX, Duration::MAX, Vec::new());
    for _ in 0..5 {
        let start = Instant::now();
        let mut translation = translate::<G>(
            &blob.bytes,
            0..blob.code_bytes,
            &[0],
            LookAhead::Full,
            optimise(),
            wcontainers_translator_test::runner::probe::<G>(),
            G::Offer::default(),
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

fn measure_benchmarks<G: GuestTest>() {
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
    size::<G>(&build_with::<G>(
        "bench",
        "-O2",
        Library::None,
        &["SCALE=1"],
    ));
    for k in chosen {
        let name = WORKLOADS[k - 1];
        let workload = format!("WORKLOAD={k}");
        let blob = build_with::<G>("bench", "-O2", Library::None, &["SCALE=1", &workload]);
        let expected = native::<G>(&blob);
        let native_time = native_seconds::<G>(&blob, runs);
        eprintln!("{name}: native {native_time:.3} s (load {})", load());
        for &(label, omit) in &variants {
            let mut best = f64::INFINITY;
            let mut counters = Vec::new();
            for _ in 0..runs {
                let (ran, machine, _, time) =
                    translated_omitting::<G>(&blob, (LookAhead::Full, optimise()), omit);
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
                let report =
                    browser::repeated::<G>(&blob, (LookAhead::Full, optimise()), omit, repeat);
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
fn measure_calls_between_modules<G: GuestTest>() {
    use wcontainers_translator_test::programs::{ran, ready_with};
    let runs: u32 = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
    let calls: u64 = std::env::var("WC_CALLS").map_or(100_000_000, |v| v.parse().unwrap());
    let define = format!("CALLS={calls}");
    let blob = build_with::<G>("calls", "-O2", Library::None, &[&define]);
    let expected = native::<G>(&blob);
    let native_time = native_seconds::<G>(&blob, runs);
    eprintln!("calls: native {native_time:.3} s (load {})", load());
    for optimise in [Optimise::Speed, Optimise::Size] {
        let mut one = f64::INFINITY;
        let mut split = f64::INFINITY;
        for _ in 0..runs {
            let (ran_one, _, _, time) =
                translated_omitting::<G>(&blob, (LookAhead::Full, optimise), Omit::default());
            assert_eq!(ran_one, expected);
            one = one.min(time.as_secs_f64());
            let mut machine = ready_with::<G>(
                &blob,
                wcontainers_translator_test::runner::probe::<G>(),
                |m| {
                    m.optimise_for(optimise);
                    m.load_split(&blob.bytes, 0..blob.code_bytes, blob.load, &[0], 1);
                },
            );
            let started = Instant::now();
            let (stopped, registers) = machine.run();
            let time = started.elapsed().as_secs_f64();
            assert_eq!(ran::<G>(&blob, &machine, stopped, &registers), expected);
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
            let report =
                browser::repeated::<G>(&blob, (LookAhead::Full, optimise), Omit::default(), 2);
            assert_eq!(report.ran, expected);
            one = report.runs.iter().fold(one, |a, &b| a.min(b));
            let report = browser::repeated_split::<G>(&blob, optimise, 2, 1);
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

#[test]
#[ignore]
fn measure_benchmarks_arm64() {
    measure_benchmarks::<Arm64>();
}

#[test]
#[ignore]
fn measure_calls_between_modules_arm64() {
    measure_calls_between_modules::<Arm64>();
}

/// x86-64's 128-by-64 long division (`programs/x86_64/divide.c`) against the CPU's own `div`: each with a
/// high half (the long path) and with none (the fast path), natively on the reference and at look-ahead none
/// in wasmtime here; their difference is the long path's own cost a division.
#[test]
#[ignore]
fn measure_long_division_x86_64() {
    let runs: u32 = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
    const ROUNDS: f64 = 50e6;
    let mut times = Vec::new();
    for high in ["HIGH=1", "HIGH=0"] {
        let blob = build_with::<X86_64>("divide", "-O2", Library::None, &[high]);
        let expected = native::<X86_64>(&blob);
        let native_time = native_seconds::<X86_64>(&blob, runs);
        let mut best = f64::INFINITY;
        for _ in 0..runs {
            let (ran, _, _, time) = translated_omitting::<X86_64>(
                &blob,
                (LookAhead::None, Optimise::Speed),
                Omit::default(),
            );
            assert_eq!(ran, expected, "{high}");
            best = best.min(time.as_secs_f64());
        }
        eprintln!(
            "{high}: native {:.2} ns a division, wasmtime at none {:.2} ns (load here {})",
            native_time / ROUNDS * 1e9,
            best / ROUNDS * 1e9,
            load()
        );
        times.push((native_time, best));
    }
    let [(native_long, ours_long), (native_fast, ours_fast)] = times[..] else {
        unreachable!()
    };
    eprintln!(
        "the long path's own cost: native {:.2} ns a division, wasmtime {:.2} ns",
        (native_long - native_fast) / ROUNDS * 1e9,
        (ours_long - ours_fast) / ROUNDS * 1e9
    );
}
