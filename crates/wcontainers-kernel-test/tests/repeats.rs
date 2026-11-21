//! A seeded run repeats exactly: threaded programs run again under the same seed give the same output,
//! warnings, counters, CPU side warnings, turn count and files left. Each seed of `schedule_seeds` is run
//! `REPEAT_RUNS` times (2 if unset), with saves at moments drawn from it for one program. The runs of a
//! seed are given the same wall clock reading at the start, an answer from outside that the seed does
//! not decide (the futexes program's realtime deadlines branch on its nanoseconds). The threads and
//! futexes programs switch threads only at system calls; the threadsched program has threads that only
//! compute, so where the seed ends their turns decides which runs next.

use std::hash::{Hash, Hasher};
use std::path::Path;

use wcontainers_kernel_test::{driver, guest, image, reference, schedule_seeds, standin};

/// Every part of a run that a seed decides, as one comparable value: each file the container left
/// by its path, mode, owner, device and a hash of its contents.
fn measured(run: &driver::Run) -> String {
    let files: Vec<String> = run
        .files
        .iter()
        .map(|file| {
            let mut hasher = std::hash::DefaultHasher::new();
            file.contents.hash(&mut hasher);
            format!(
                "{} {:o} {}:{} {} {} {:016x}",
                String::from_utf8_lossy(&file.path),
                file.mode,
                file.uid,
                file.gid,
                file.rdev,
                file.contents.len(),
                hasher.finish()
            )
        })
        .collect();
    format!(
        "{:?}\n{:?}\n{:?}\n{:?}\nsaved {} turns {}\nfiles:\n{}",
        run.outcome,
        run.warnings,
        run.cpu_warnings,
        run.counters,
        run.saved,
        run.turns,
        files.join("\n")
    )
}

/// Whether a thread left `run` because its turn ended and another thread ran next, on the
/// WebAssembly back end, which warns of such a leave once.
#[cfg(feature = "runtime")]
fn preempted(run: &driver::Run) -> bool {
    use wcontainers_executor_wasmtime::reason;
    run.cpu_warnings
        .iter()
        .any(|w| w.reason == reason::LEFT_ANOTHER_THREAD && w.detail == reason::NOT_A_CALL)
}

/// `preempting`: the program's threads share the CPU by turns ending at the seed's limits, which the
/// WebAssembly back end must show.
fn repeats(name: &str, saves: usize, preempting: bool) {
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
        root_image: None,
        address_space: standin::SPACE_32_BIT,
    };
    let runs: u64 = std::env::var("REPEAT_RUNS")
        .ok()
        .map_or(2, |runs| runs.parse().expect("REPEAT_RUNS is a number"));
    for seed in schedule_seeds() {
        let wall_start = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the clock is after 1970")
            .as_nanos() as u64;
        let injections = driver::Injections {
            saves,
            wall_start: Some(wall_start),
            ..Default::default()
        };
        let run = || driver::run_injected(&container, seed, injections).expect("an outcome");
        let kept = run();
        let first = measured(&kept);
        for again in 1..runs {
            let other = measured(&run());
            assert_eq!(first, other, "{name}, seed {seed}, run {again}");
        }
        #[cfg(feature = "runtime")]
        {
            let preempted = preempted(&kept);
            println!("{name} seed {seed}: another thread ran after a turn ended: {preempted}");
            assert!(
                !preempting || preempted,
                "{name}, seed {seed}: no turn ended mid-run"
            );
        }
        #[cfg(not(feature = "runtime"))]
        let _ = preempting;
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
    repeats("threads", 0, false);
}

#[test]
fn futexes_repeat_for_a_seed() {
    repeats("futexes", 0, false);
}

#[test]
fn threads_saved_and_brought_back_repeat_for_a_seed() {
    repeats("threads", 3, false);
}

#[test]
fn preempted_threads_repeat_for_a_seed() {
    repeats("threadsched", 0, true);
}

#[test]
fn preempted_threads_saved_and_brought_back_repeat_for_a_seed() {
    repeats("threadsched", 3, true);
}
