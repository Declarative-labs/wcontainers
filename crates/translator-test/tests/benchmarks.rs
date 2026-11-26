//! Not a check: times the benchmark set (`programs/bench.c` at full size, -O2) natively and at full
//! look-ahead in wasmtime and Chrome, with and without each check, and prints the size of the translation.
//! Each is written once over the guest and run for each (`measure_benchmarks_arm64`). Run with `--ignored
//! --nocapture` in a release build. Knobs: `WC_BENCH_RUNS` (best of, default 3),
//! `WC_BENCH_WORKLOADS` (numbers from bench.c, default all), `WC_BENCH_VARIANTS` (which of the six check
//! variants, numbered from 1, default all), `WC_BENCH_CHROME` (0 leaves Chrome out), `WC_BENCH_REPEAT` (how
//! many times Chrome runs the program in the same instances, each run timed by itself, default 1),
//! `WC_CHROME_JS_FLAGS` (V8 flags for Chrome) and `WC_OPTIMISE` (`size` translates for size, default speed);
//! the runners' access ordering setting chooses the form, and `measure_ordered_forms_x86_64` times the forms
//! against each other.

use std::io::Write;
use std::time::{Duration, Instant};
use wcontainers_translator::{LookAhead, Omit, Optimise, translate};

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

#[test]
#[ignore]
fn measure_benchmarks_arm64() {
    arm64::measure_benchmarks();
}

#[test]
#[ignore]
fn measure_calls_between_modules_arm64() {
    arm64::measure_calls_between_modules();
}

/// The benchmark set built for x86-64: natively on the x86 reference, and translated at full here.
#[test]
#[ignore]
fn measure_benchmarks_x86_64() {
    x86_64::measure_benchmarks();
}

/// The benchmark set built for x86-64, run on this machine plainly and through the x86-64 native output
/// (docs/x86-native-output.md section 8), its pieces at full look-ahead linked in a blob of their own, without and
/// with the count, for a host process and in process with each checked form; best of `WC_BENCH_RUNS` (default 5)
/// each, with the geometric mean of the ratios to the host's run and, in process, to the host process's run (the
/// unchecked form). Linux x86-64 only, where the reference is this machine. `WC_BENCH_WORKLOADS` as above;
/// `WC_BENCH_WAYS` names some of `process`, `wrapping` and `exact` (default all).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore]
fn measure_benchmarks_natively_x86_64() {
    use wcontainers_translator_test::reference::with_reference;
    use wcontainers_translator_test::x86_64::part::native_programs::run_timed;
    use wcontainers_x86::native::{Check, Way};
    let runs: u32 = std::env::var("WC_BENCH_RUNS").map_or(5, |v| v.parse().unwrap());
    let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS").map_or((1..=8).collect(), |v| {
        v.split(',').map(|n| n.parse().unwrap()).collect()
    });
    let named = std::env::var("WC_BENCH_WAYS").ok();
    let ways: Vec<(&str, Way)> = [
        ("process", Way::Process),
        ("wrapping", Way::InProcess(Check::Wrapping)),
        ("exact", Way::InProcess(Check::Exact)),
    ]
    .into_iter()
    .filter(|(name, _)| {
        named
            .as_deref()
            .is_none_or(|n| n.split(',').any(|n| n == *name))
    })
    .collect();
    // By way and count: the ratios to the host's run, and to the host process's.
    let mut ratios = vec![[(Vec::new(), Vec::new()), (Vec::new(), Vec::new())]; ways.len()];
    for k in chosen {
        let name = WORKLOADS[k - 1];
        let workload = format!("WORKLOAD={k}");
        let blob = x86_64::build_with(
            "bench",
            "-O2",
            x86_64::Library::None,
            &["SCALE=1", &workload],
            x86_64::Position::Fixed,
        );
        let expected = x86_64::native(&blob);
        let plain = x86_64::native_seconds(&blob, runs);
        eprintln!("{name}: on the host {plain:.3} s (load {})", x86_64::load());
        let mut unchecked = [None, None];
        for ((way_name, way), ratios) in ways.iter().zip(&mut ratios) {
            for ((to_host, to_unchecked), counted) in ratios.iter_mut().zip([false, true]) {
                let (ours, seconds) = with_reference(
                    &wcontainers_translator_test::x86_64::part::REFERENCE,
                    |reference| run_timed(reference, &blob, (*way, LookAhead::Full), counted, runs),
                )
                .unwrap_or_else(|e| panic!("{name} {way_name}: {e}"));
                assert_eq!(ours.ran, expected, "{name} {way_name}, counted {counted}");
                let ratio = seconds / plain;
                to_host.push(ratio);
                let against = match (way, unchecked[usize::from(counted)]) {
                    (Way::Process, _) => {
                        unchecked[usize::from(counted)] = Some(seconds);
                        String::new()
                    }
                    (_, Some(p)) => {
                        to_unchecked.push(seconds / p);
                        format!(", {:.3} times the unchecked form's", seconds / p)
                    }
                    (_, None) => String::new(),
                };
                eprintln!(
                    "{name}: native output {way_name} at full, counted {counted}: {seconds:.3} s, {ratio:.3} times the host's{against} ({} pieces, made in {} runs; load {})",
                    ours.pieces,
                    ours.runs,
                    x86_64::load()
                );
            }
        }
    }
    let mean = |r: &[f64]| (r.iter().map(|r| r.ln()).sum::<f64>() / r.len() as f64).exp();
    let most = |r: &[f64]| r.iter().copied().fold(0.0, f64::max);
    for ((way_name, _), ratios) in ways.iter().zip(&ratios) {
        for ((to_host, to_unchecked), counted) in ratios.iter().zip([false, true]) {
            eprintln!(
                "native output {way_name} at full, counted {counted}: geometric mean {:.3} times the host's, the most {:.3}",
                mean(to_host),
                most(to_host)
            );
            if !to_unchecked.is_empty() {
                eprintln!(
                    "native output {way_name} at full, counted {counted}: geometric mean {:.3} times the unchecked form's, the most {:.3}",
                    mean(to_unchecked),
                    most(to_unchecked)
                );
            }
        }
    }
}

/// Times each ordered form against the default form, the x86-64 benchmark set in wasmtime here, not a check.
///
/// No native run, so a host of any CPU times it. Each of `WC_BENCH_RUNS` rounds (default 5) runs every form once,
/// each result equal to the default form's; printed are each form's best and median time and the median of its
/// ratio to the default form within a round, then their geometric mean, with the load.
#[test]
#[ignore]
fn measure_ordered_forms_x86_64() {
    use wcontainers_translator::{AccessOrdering, OrderedAccess};
    use wcontainers_translator_test::x86_64::programs::{ran, ready_with};
    let runs: usize = std::env::var("WC_BENCH_RUNS").map_or(5, |v| v.parse().unwrap());
    let chosen: Vec<usize> = std::env::var("WC_BENCH_WORKLOADS").map_or((1..=8).collect(), |v| {
        v.split(',').map(|n| n.parse().unwrap()).collect()
    });
    let kind = OrderedAccess::SequentiallyConsistent;
    let forms = [
        AccessOrdering::Unordered,
        AccessOrdering::Ordered(kind),
        AccessOrdering::LoadsOrdered(kind),
    ];
    let median = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let mean = |r: &[f64]| (r.iter().map(|r| r.ln()).sum::<f64>() / r.len() as f64).exp();
    // By form, each workload's median ratio to the default form.
    let mut ratios = vec![Vec::new(); forms.len()];
    eprintln!("load at the start {}", x86_64::load());
    for k in chosen {
        let name = WORKLOADS[k - 1];
        let workload = format!("WORKLOAD={k}");
        let blob = x86_64::build_with(
            "bench",
            "-O2",
            x86_64::Library::None,
            &["SCALE=1", &workload],
            x86_64::Position::Fixed,
        );
        let mut times = vec![Vec::new(); forms.len()];
        let mut expected = None;
        for _ in 0..runs {
            for (form, times) in forms.iter().zip(&mut times) {
                let host = wcontainers_translator_test::x86_64::runner::probe();
                let mut machine = ready_with(&blob, host, |machine| {
                    machine.order_accesses(*form);
                    machine.load_omitting(
                        &blob.bytes,
                        0..blob.code_bytes,
                        (blob.load, blob.link_address),
                        &[0],
                        LookAhead::Full,
                        Omit::default(),
                    );
                });
                let started = Instant::now();
                let (stopped, registers) = machine.run();
                times.push(started.elapsed().as_secs_f64());
                let result = ran(&blob, &machine, stopped, &registers);
                match &expected {
                    Some(expected) => assert_eq!(&result, expected, "{name} in {form:?}"),
                    None => expected = Some(result),
                }
            }
        }
        for (i, form) in forms.iter().enumerate() {
            let within: Vec<f64> = times[i].iter().zip(&times[0]).map(|(t, d)| t / d).collect();
            let ratio = median(within);
            ratios[i].push(ratio);
            let best = times[i].iter().copied().fold(f64::INFINITY, f64::min);
            eprintln!(
                "{name}: {form:?}: best {best:.3} s, median {:.3} s, {ratio:.3} times the default form (median of {runs} rounds; load {})",
                median(times[i].clone()),
                x86_64::load()
            );
        }
    }
    for (form, ratios) in forms.iter().zip(&ratios) {
        eprintln!(
            "{form:?}: geometric mean {:.3} times the default form, the most {:.3} (load {})",
            mean(ratios),
            ratios.iter().copied().fold(0.0, f64::max),
            x86_64::load()
        );
    }
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
        let blob = x86_64::build_with(
            "divide",
            "-O2",
            x86_64::Library::None,
            &[high],
            x86_64::Position::Fixed,
        );
        let expected = x86_64::native(&blob);
        let native_time = x86_64::native_seconds(&blob, runs);
        let mut best = f64::INFINITY;
        for _ in 0..runs {
            let (ran, _, _, time) = x86_64::translated_omitting(
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
            x86_64::load()
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

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
#[allow(
    clippy::unit_arg,
    reason = "ARM64 offers no feature lists, so its offer is (), passed where x86-64 passes its list"
)]
mod arm64 {
    use super::*;
    use wcontainers_translator_test::arm64 as harness;
    use wcontainers_translator_test::arm64::Cpu;
    pub(super) use wcontainers_translator_test::arm64::browser;
    pub(super) use wcontainers_translator_test::arm64::programs::{
        Blob, Library, Position, build_with, load, native, native_seconds, translated_omitting,
    };
    pub(super) use wcontainers_translator_test::arm64::runner::compile_time;

    include!("for_guest/benchmarks.rs");

    /// Not a check: what a direct call costs when it crosses from one code module to another, in wasmtime and
    /// Chrome (`programs/calls.c`, -O2, `WC_CALLS` calls of a small function, default 100 million): the program
    /// in one module, and with every piece in a module of its own, so that each call goes through the lookup.
    /// Best of `WC_BENCH_RUNS`, for both settings; Chrome runs the program twice in the same instances, and
    /// `WC_BENCH_CHROME=0` leaves it out.
    pub(super) fn measure_calls_between_modules() {
        use harness::programs::{ran, ready_with};
        let runs: u32 = std::env::var("WC_BENCH_RUNS").map_or(3, |v| v.parse().unwrap());
        let calls: u64 = std::env::var("WC_CALLS").map_or(100_000_000, |v| v.parse().unwrap());
        let define = format!("CALLS={calls}");
        let blob = build_with("calls", "-O2", Library::None, &[&define], Position::Fixed);
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
                let mut machine = ready_with(&blob, harness::runner::probe(), |m| {
                    m.optimise_for(optimise);
                    m.load_split(
                        &blob.bytes,
                        0..blob.code_bytes,
                        (blob.load, blob.link_address),
                        &[0],
                        1,
                    );
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
                let report =
                    browser::repeated(&blob, (LookAhead::Full, optimise), Omit::default(), 2);
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
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    use wcontainers_translator_test::x86_64 as harness;
    use wcontainers_translator_test::x86_64::Cpu;
    pub(super) use wcontainers_translator_test::x86_64::browser;
    pub(super) use wcontainers_translator_test::x86_64::programs::{
        Blob, Library, Position, build_with, load, native, native_seconds, translated_omitting,
    };
    pub(super) use wcontainers_translator_test::x86_64::runner::compile_time;

    include!("for_guest/benchmarks.rs");
}
