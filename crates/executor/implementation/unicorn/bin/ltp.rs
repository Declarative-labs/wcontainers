//! Runs the Linux Test Project in the reference or on our kernel, and compares results files.
//!
//! ltp reference <results file> [group | group:test | test]...
//! ltp ours <results file> [group | group:test | test]...
//! ltp compare <reference results file> <our results file>
//!
//! With no tests named, the whole suite runs. `LTP_TAR` is the image's files as `ltp/build.sh`
//! leaves them; `LTP_JOBS` how many tests run side by side (4); `LTP_SEED` the seed of our runs'
//! schedules (1); `LTP_LIMIT` the seconds a test may run before it is stopped (900 on our
//! kernel, 1800 in the reference). Each test's output goes under `<results file>.logs/`. A
//! results file that exists is continued: the tests it has are not run again.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use wcontainers_executor_unicorn::libctest;
use wcontainers_executor_unicorn::ltp::{self, Line};

fn number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("{name} is not a number"))
        })
        .unwrap_or(default)
}

fn usage() -> ! {
    eprintln!(
        "usage: ltp reference|ours <results file> [group | group:test | test]...\n       \
         ltp compare <reference results file> <our results file>"
    );
    std::process::exit(2)
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let [mode, id, log] = &arguments[..]
        && mode == ltp::ONE_TEST
    {
        let tar = PathBuf::from(std::env::var("LTP_TAR").expect("LTP_TAR, as the run had it"));
        let seed = number(ltp::SEED_VARIABLE, 1);
        let limit = Duration::from_secs(number(ltp::LIMIT_VARIABLE, 900));
        ltp::run_one_test(&tar, id, Path::new(log), seed, limit);
    }
    let [mode, results, selection @ ..] = &arguments[..] else {
        usage()
    };
    if mode == "compare" {
        let [ours] = selection else { usage() };
        let reference = ltp::read_results(Path::new(results));
        let ours = ltp::read_results(Path::new(ours));
        print!("{}", ltp::compare(&reference, &ours));
        return;
    }
    let tar = PathBuf::from(std::env::var("LTP_TAR").unwrap_or_else(|_| {
        eprintln!("LTP_TAR names the image's files that ltp/build.sh left");
        std::process::exit(2)
    }));
    let results = PathBuf::from(results);
    let done: Vec<String> = if results.exists() {
        ltp::read_results(&results)
            .into_iter()
            .map(|line| line.id)
            .collect()
    } else {
        Vec::new()
    };
    let tests: Vec<ltp::Test> = ltp::select(&ltp::groups(&tar), selection)
        .into_iter()
        .filter(|test| !done.contains(&test.id()))
        .collect();
    let jobs = number("LTP_JOBS", 4) as usize;
    let logs = PathBuf::from(format!("{}.logs", results.display()));
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&results)
        .expect("the results file");
    let mut report = |line: &Line| {
        out.write_all(line.to_text().as_bytes())
            .expect("writing the results file");
        eprint!("{}", line.to_text());
    };
    eprintln!("{} tests to run, {} done before", tests.len(), done.len());
    match mode.as_str() {
        "reference" => {
            let limit = Duration::from_secs(number("LTP_LIMIT", 1800));
            let report = std::sync::Mutex::new(&mut report);
            libctest::each(jobs, &tests, |test| {
                let line = ltp::run_reference(test, limit, &logs);
                (report.lock().expect("no worker panicked"))(&line);
                line
            });
        }
        "ours" => {
            let limit = Duration::from_secs(number(ltp::LIMIT_VARIABLE, 900));
            let seed = number(ltp::SEED_VARIABLE, 1);
            ltp::run_ours(&tests, seed, jobs, limit, &logs, report);
        }
        _ => usage(),
    }
}
