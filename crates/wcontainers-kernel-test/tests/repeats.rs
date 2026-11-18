//! A seeded run repeats exactly: threaded programs run again under the same seed give the same output,
//! warnings, counters, CPU side warnings and turn count. Each seed of `schedule_seeds` is run
//! `REPEAT_RUNS` times (2 if unset), with saves at moments drawn from it for one program.

use std::hash::{Hash, Hasher};
use std::path::Path;

use wcontainers_kernel_test::{driver, guest, image, reference, schedule_seeds, standin};

/// Every part of a run that a seed decides, as one comparable value.
fn measured(run: &driver::Run) -> String {
    format!(
        "{:?}\n{:?}\n{:?}\n{:?}\nsaved {} turns {}",
        run.outcome, run.warnings, run.cpu_warnings, run.counters, run.saved, run.turns
    )
}

fn repeats(name: &str, saves: usize) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build(name, &[], &out);
    let directory = program.parent().expect("a directory");
    let path = format!("/wc/{name}");
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(directory, "wc"));
    let container = driver::Container {
        files: &files,
        path: &path,
        arguments: &[path.as_str()],
        environment: reference::ENVIRONMENT,
        working_directory: "/",
        backend: None,
        cached: false,
        published: &[],
        address_space: standin::SPACE_32_BIT,
    };
    let runs: u64 = std::env::var("REPEAT_RUNS")
        .ok()
        .map_or(2, |runs| runs.parse().expect("REPEAT_RUNS is a number"));
    for seed in schedule_seeds() {
        let injections = driver::Injections {
            saves,
            ..Default::default()
        };
        let run = || driver::run_injected(&container, seed, injections).expect("an outcome");
        let kept = run();
        let first = measured(&kept);
        for again in 1..runs {
            let other = measured(&run());
            assert_eq!(first, other, "{name}, seed {seed}, run {again}");
        }
        let mut hasher = std::hash::DefaultHasher::new();
        first.hash(&mut hasher);
        println!(
            "{name} seed {seed}: {runs} runs the same; {} turns, saved {}, {} CPU side warnings, \
             fingerprint {:016x}",
            kept.turns,
            kept.saved,
            kept.cpu_warnings.len(),
            hasher.finish()
        );
    }
}

#[test]
fn threads_repeat_for_a_seed() {
    repeats("threads", 0);
}

#[test]
fn futexes_repeat_for_a_seed() {
    repeats("futexes", 0);
}

#[test]
fn threads_saved_and_brought_back_repeat_for_a_seed() {
    repeats("threads", 3);
}
