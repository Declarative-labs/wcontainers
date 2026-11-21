//! musl's own test suite, libc-test (MIT; `libc-test/` holds its common, functional and regression
//! tests from https://repo.or.cz/libc-test.git at commit 63e36e9, less those that load shared
//! libraries), built as static ARM64 programs and compared with the reference.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::driver::Outcome;

/// One test program of the suite.
#[derive(Clone, Debug)]
pub struct Test {
    /// Its directory in the suite and its name, as `functional-pthread_cond`.
    pub name: String,
    /// The suite's runner (`runtest`), which runs the test as its child and reports how it ended,
    /// in a directory of its own with the test, which runs mount at `/wc`.
    pub runner: PathBuf,
}

/// How long the runner lets a test go on before it kills it: long enough for the stand-in CPU on a
/// loaded machine, so that only a test that never ends is reported as timed out.
pub const TIMEOUT_SECONDS: &str = "600";

impl Test {
    /// The runner's arguments after its name.
    pub fn arguments(&self) -> [String; 3] {
        [
            String::from("-t"),
            String::from(TIMEOUT_SECONDS),
            format!("/wc/{}", self.name),
        ]
    }
}

/// Where the suite's sources are.
pub fn source_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("libc-test/src")
}

/// How many builds or runs go side by side: `LIBC_TEST_JOBS`, 4 if unset.
pub fn jobs() -> usize {
    std::env::var("LIBC_TEST_JOBS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(4)
}

/// Runs `work` on every item with `jobs` items at a time, and returns the results in order.
pub fn each<T: Sync, R: Send>(jobs: usize, items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let results = Mutex::new((0..items.len()).map(|_| None).collect::<Vec<Option<R>>>());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(items.len()).max(1) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else { break };
                    let result = work(item);
                    results.lock().expect("no worker panicked")[index] = Some(result);
                }
            });
        }
    });
    results
        .into_inner()
        .expect("no worker panicked")
        .into_iter()
        .map(|result| result.expect("every item ran"))
        .collect()
}

/// The compiler flags of the suite's `config.mak.def`, for zig's C compiler.
const FLAGS: &[&str] = &[
    "-target",
    "aarch64-linux-musl",
    "-O2",
    "-std=c99",
    "-D_POSIX_C_SOURCE=200809L",
    "-Wno-unused-function",
    "-Wno-missing-braces",
    "-Wno-unused",
    "-Wno-overflow",
    "-Wno-unknown-pragmas",
    "-Wno-literal-range",
    "-fno-builtin",
    "-frounding-math",
];

/// Builds every test whose name `wanted` accepts into `out`, each in its own directory.
pub fn build(out: &Path, wanted: impl Fn(&str) -> bool) -> Vec<Test> {
    let sources = source_directory();
    let common = out.join("common");
    std::fs::create_dir_all(&common).expect("creating the output directory");
    let mut tests = Vec::new();
    for directory in ["functional", "regression"] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(sources.join(directory))
            .expect("the suite's sources")
            .map(|entry| entry.expect("a directory entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "c"))
            .collect();
        files.sort();
        for file in files {
            let stem = file
                .file_stem()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            // A part of another test, linked into it.
            if stem.ends_with("_dso") {
                continue;
            }
            let name = format!("{directory}-{stem}");
            if wanted(&name) {
                tests.push((name, file));
            }
        }
    }
    let common_sources = common_sources(&sources);
    let objects: Vec<PathBuf> = each(jobs(), &common_sources, |source| {
        let object = common.join(format!(
            "{}.o",
            source.file_stem().expect("a name").to_string_lossy()
        ));
        zig(Command::new("zig")
            .arg("cc")
            .args(FLAGS)
            .arg("-I")
            .arg(sources.join("common"))
            .arg("-c")
            .arg("-o")
            .arg(&object)
            .arg(source));
        object
    });
    let (runner_objects, objects): (Vec<PathBuf>, Vec<PathBuf>) = objects
        .into_iter()
        .partition(|object| object.ends_with("runtest.o"));
    let runner = common.join("runtest");
    zig(Command::new("zig")
        .arg("cc")
        .args(FLAGS)
        .arg("-static")
        .arg("-o")
        .arg(&runner)
        .args(&runner_objects)
        .args(&objects));
    each(jobs(), &tests, |(name, source)| {
        let directory = out.join(name);
        std::fs::create_dir_all(&directory).expect("creating the test's directory");
        let program = directory.join(name);
        let mut command = Command::new("zig");
        command
            .arg("cc")
            .args(FLAGS)
            .arg("-I")
            .arg(sources.join("common"))
            .arg("-static")
            .arg("-o")
            .arg(&program)
            .arg(source);
        // `tls_align` checks thread-local data of another object, linked in when static.
        if name == "functional-tls_align" {
            command.arg(sources.join("functional/tls_align_dso.c"));
        }
        command.args(&objects).args(["-lpthread", "-lm", "-lrt"]);
        zig(&mut command);
        let test_runner = directory.join("runtest");
        std::fs::copy(&runner, &test_runner).expect("copying the runner");
        Test {
            name: name.clone(),
            runner: test_runner,
        }
    })
}

fn common_sources(sources: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(sources.join("common"))
        .expect("the suite's common sources")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "c"))
        .collect();
    files.sort();
    files
}

fn zig(command: &mut Command) {
    let output = command
        .output()
        .expect("running zig cc; run the tests inside `nix develop`");
    assert!(
        output.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// What a test said and how it ended: its output (the suite prints only on failure) and its exit
/// code.
pub fn verdict(outcome: &Outcome) -> String {
    format!(
        "exit {} stdout {:?} stderr {:?}",
        outcome.exit_code,
        String::from_utf8_lossy(&outcome.stdout),
        String::from_utf8_lossy(&outcome.stderr)
    )
}
