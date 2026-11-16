//! Runs wcontainers-kernel on a stand-in CPU and compares it with real ARM64 Linux.

pub mod buffer;
pub mod busybox;
pub mod driver;
pub mod generate;
pub mod guest;
pub mod image;
pub mod layer1;
pub mod reference;
pub mod standin;

use std::path::Path;

/// Runs a program from a host directory on our kernel and in the reference, with the same arguments, in
/// a container of the reference image with the directory at `/wc`, and returns both.
pub fn run_both(program: &Path, arguments: &[&str]) -> (driver::Run, driver::Outcome) {
    let name = format!(
        "/wc/{}",
        program
            .file_name()
            .expect("program has a name")
            .to_string_lossy()
    );
    let mut all = vec![name.as_str()];
    all.extend_from_slice(arguments);
    let ours = run_ours(
        program.parent().expect("program has a directory"),
        &name,
        &all,
    );
    let theirs = reference::run(program, arguments);
    (ours, theirs)
}

/// Runs `path` with `arguments` (the first is its name) in a container of the reference image with
/// `directory` at `/wc`.
pub fn run_ours(directory: &Path, path: &str, arguments: &[&str]) -> driver::Run {
    run_ours_saving(directory, path, arguments, 0, 0)
}

/// As [`run_ours`], with the run's seed and saved and brought back `saves` times.
pub fn run_ours_saving(
    directory: &Path,
    path: &str,
    arguments: &[&str],
    seed: u64,
    saves: usize,
) -> driver::Run {
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(directory, "wc"));
    let container = driver::Container {
        files: &files,
        path,
        arguments,
        environment: reference::ENVIRONMENT,
        working_directory: "/",
    };
    driver::run_saving(&container, seed, saves)
        .unwrap_or_else(|failure| panic!("no outcome on our kernel: {failure:?}"))
}

/// Builds the guest program `name` into `out`, runs it with `arguments` on our kernel and in the
/// reference, and checks that both print the same lines and end the same way; returns our run for its
/// counters.
pub fn assert_same_output(out: &Path, name: &str, arguments: &[&str]) -> driver::Run {
    let program = guest::build(name, &[], out);
    let (ours, theirs) = run_both(&program, arguments);
    let ours_text = String::from_utf8_lossy(&ours.outcome.stdout).into_owned();
    let theirs_text = String::from_utf8_lossy(&theirs.stdout).into_owned();
    let differences: Vec<String> = ours_text
        .lines()
        .zip(theirs_text.lines())
        .filter(|(our, their)| our != their)
        .map(|(our, their)| format!("ours {our:?}\ntheirs {their:?}"))
        .collect();
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    assert_eq!(ours.outcome, theirs);
    ours
}

/// The seeds of the schedules a threaded program runs under: `SCHEDULE_SEEDS` of them (3 if unset)
/// from `SCHEDULE_SEED` on (1 if unset). Each seed decides where every turn on the CPU ends.
pub fn schedule_seeds() -> std::ops::Range<u64> {
    let number = |name: &str, default: u64| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let first = number("SCHEDULE_SEED", 1);
    first..first + number("SCHEDULE_SEEDS", 3)
}

/// As [`assert_same_output`], with ours run under every seed of [`schedule_seeds`], and with
/// `saves` saves at moments drawn from each seed; returns our runs.
pub fn assert_same_output_seeded(
    out: &Path,
    name: &str,
    arguments: &[&str],
    saves: usize,
) -> Vec<driver::Run> {
    let program = guest::build(name, &[], out);
    let theirs = reference::run(&program, arguments);
    let path = format!(
        "/wc/{}",
        program.file_name().expect("a name").to_string_lossy()
    );
    let mut all = vec![path.as_str()];
    all.extend_from_slice(arguments);
    let directory = program.parent().expect("program has a directory");
    let mut runs = Vec::new();
    for seed in schedule_seeds() {
        let ours = run_ours_saving(directory, &path, &all, seed, saves);
        let ours_text = String::from_utf8_lossy(&ours.outcome.stdout).into_owned();
        let theirs_text = String::from_utf8_lossy(&theirs.stdout).into_owned();
        let differences: Vec<String> = ours_text
            .lines()
            .zip(theirs_text.lines())
            .filter(|(our, their)| our != their)
            .map(|(our, their)| format!("seed {seed}: ours {our:?}\ntheirs {their:?}"))
            .collect();
        assert!(differences.is_empty(), "{}", differences.join("\n"));
        assert_eq!(ours.outcome, theirs, "seed {seed}");
        runs.push(ours);
    }
    runs
}
