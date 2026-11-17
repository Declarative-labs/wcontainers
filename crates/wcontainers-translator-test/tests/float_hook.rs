//! Not a check by itself: times `programs/rounding.c`, whose floating point runs under rounding modes other
//! than the default and so takes the float hook on every operation, at look-ahead none and full in
//! wasmtime and in Chrome, and checks its output against the native run. Run with `--ignored --nocapture`
//! in a release build. Knobs: `WC_FLOAT_STEPS` (loop turns per mode, default 200000) and `WC_FLOAT_RUNS`
//! (best of, default 3).

use wcontainers_translator::{LookAhead, Omit, Optimise};
use wcontainers_translator_test::browser;
use wcontainers_translator_test::programs::{
    Blob, Library, build_with, native, translated_omitting,
};

fn knob(name: &str, default: u32) -> u32 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

fn rounding() -> Blob {
    let steps = format!("STEPS={}", knob("WC_FLOAT_STEPS", 200_000));
    build_with("rounding", "-O2", Library::C, &[&steps])
}

#[test]
#[ignore]
fn float_hook_time() {
    let runs = knob("WC_FLOAT_RUNS", 3);
    let blob = rounding();
    let theirs = native(&blob);
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let mut best = f64::MAX;
        for _ in 0..runs {
            let (ours, _, _, time) =
                translated_omitting(&blob, (look_ahead, Optimise::Speed), Omit::default());
            assert_eq!(ours, theirs, "{look_ahead:?}");
            best = best.min(time.as_secs_f64());
        }
        eprintln!("float hook, {look_ahead:?}: best of {runs} {best:.4} s");
    }
}

#[test]
#[ignore]
fn float_hook_time_in_chrome() {
    let runs = knob("WC_FLOAT_RUNS", 3);
    let blob = rounding();
    let theirs = native(&blob);
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let how = (look_ahead, Optimise::Speed);
        let report = browser::repeated(&blob, how, Omit::default(), runs);
        assert_eq!(report.ran.stdout, theirs.stdout, "{look_ahead:?}");
        assert_eq!(report.ran.status, theirs.status, "{look_ahead:?}");
        let best = report.runs.iter().copied().fold(f64::MAX, f64::min);
        eprintln!("float hook in Chrome, {look_ahead:?}: best of {runs} {best:.4} s");
    }
}
