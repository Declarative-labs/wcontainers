//! The fuzzing runs: a lying implementation met by a correct game, a hostile game met by the front and by
//! pass-through, and both at once. Each run is one process: `FUZZ_MODE` (liar-correct, front-hostile, pass-hostile,
//! liar-hostile, or front-correct and pass-correct as a base), `FUZZ_SEED`, `FUZZ_CALLS`, `FUZZ_THREADS`. A crash or a
//! sanitizer report ends the process; a watchdog turns 60 s with no call returning into a failure (exit 3). The
//! correct game counts every answer that breaks a promise the spike's glue made; the count is printed, and the run
//! fails on it only where the generated glue makes that promise (FUZZ_PROMISES=all fails on any).
#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

use wcontainers_direct3d_11_direct_harness::front::Front;
use wcontainers_direct3d_11_direct_harness::liar::{self, Liar, MISCHIEF, PANIC};
use wcontainers_direct3d_11_direct_harness::{Driver, Entry, fuzz_achieved, fuzz_calls, fuzz_violations, live_fake_objects};

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
}

/// Ends the process when no call has returned for `limit`, which is a hang.
fn watchdog(limit: Duration) {
    std::thread::spawn(move || {
        let (mut last, mut since) = (fuzz_calls(), Instant::now());
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let now = fuzz_calls();
            if now != last {
                (last, since) = (now, Instant::now());
            } else if since.elapsed() > limit {
                println!("FUZZ HANG: no call returned for {} s, at {now} calls", limit.as_secs());
                std::process::exit(3);
            }
        }
    });
}

#[test]
#[ignore = "a fuzzing run, run on its own with its settings"]
fn fuzz() {
    let mode = std::env::var("FUZZ_MODE").unwrap_or_else(|_| "liar-correct".into());
    let (seed, calls, threads) = (setting("FUZZ_SEED", 1), setting("FUZZ_CALLS", 100_000), setting("FUZZ_THREADS", 8) as u32);
    liar::seed(seed);
    // The liar's panics are its lies, caught by the glue: not printed; any other panic is.
    let printing = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.payload().downcast_ref::<&str>() != Some(&PANIC) {
            printing(info);
        }
    }));
    watchdog(Duration::from_secs(60));
    let (entry, driver) = match mode.as_str() {
        "liar-correct" => (Entry::served(Liar), Driver::Correct),
        "liar-hostile" => (Entry::served(Liar), Driver::Hostile),
        "front-hostile" => (Entry::served(Front), Driver::Hostile),
        "pass-hostile" => (Entry::pass_through(), Driver::Hostile),
        "front-correct" => (Entry::served(Front), Driver::Correct),
        "pass-correct" => (Entry::pass_through(), Driver::Correct),
        other => panic!("no mode {other}"),
    };
    let started = Instant::now();
    let made = entry.fuzz(driver, seed, calls, threads);
    let (violations, first) = fuzz_violations();
    let [devices, resources, mappings, views, results, back_buffers, tables] = fuzz_achieved();
    println!(
        "FUZZ {mode} seed {seed} achieved: {devices} devices, {resources} resources (and other objects, hostile), \
         {mappings} mappings written, {views} views, {results} query results, {back_buffers} back buffers, \
         {} of 8 kinds of table reached",
        tables.count_ones()
    );
    println!(
        "FUZZ {mode} seed {seed}: {made} calls on {threads} threads in {:.1} s; promises broken {violations} {first}; \
         liar mischief {}; fake objects alive {}",
        started.elapsed().as_secs_f64(),
        MISCHIEF.load(std::sync::atomic::Ordering::Relaxed),
        live_fake_objects(),
    );
    assert!(made >= calls * 9 / 10, "too few calls made");
    if std::env::var("FUZZ_PROMISES").is_ok_and(|value| value == "all") || mode != "liar-correct" {
        assert_eq!(violations, 0, "a promise broken: {first}");
    }
    if mode == "pass-correct" {
        assert_eq!(live_fake_objects(), 0, "every system object given back through both glues");
    }
}
