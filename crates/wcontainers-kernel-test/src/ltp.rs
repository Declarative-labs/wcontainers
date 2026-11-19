//! The Linux Test Project (layer 3), built by `ltp/build.sh` into an image whose files `docker
//! export` gives. Each test runs as the first process of a container of its own, in the reference
//! and on our kernel, and its verdict goes into a results file; two results files are compared
//! test by test.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::{driver, image, reference};

/// The pinned release `ltp/build.sh` builds.
pub const RELEASE: &str = "20260930";

/// The image the build makes, with the suite at `/opt/ltp`.
pub fn image_name() -> String {
    format!("wc-kernel-ltp:{RELEASE}")
}

/// The image `ltp/build.sh` builds for our kernel's root: the suite's image's files at the
/// backend's directory, served as the container's root, as a container's root is its image's.
pub fn served_image_name() -> String {
    format!("wc-kernel-ltp-served:{RELEASE}")
}

/// The groups (the suite's `runtest` files) a whole run leaves out: crashme, whose crash02
/// forks without bound and would exhaust the process numbers of the reference's machine, which
/// other containers share. Every other test says what it needs and skips itself when that is
/// missing.
pub const LEFT_OUT: &[&str] = &["crashme"];

/// The environment of every test: a default container's, with the suite's programs on the path
/// and its root and temporary directory named as its runners name them.
pub const ENVIRONMENT: &[&str] = &[
    "PATH=/opt/ltp/testcases/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    "HOSTNAME=wc-kernel",
    "HOME=/root",
    "LTPROOT=/opt/ltp",
    "TMPDIR=/tmp",
];

/// The suite's variable that scales its own time limits for a slow machine.
const TIMEOUT_MULTIPLIER: &str = "LTP_TIMEOUT_MUL";

/// What joins an environment variable's name to its value.
const ASSIGNMENT: &str = "=";

/// tst_test.h's `DEFAULT_TIMEOUT`: the suite's own limit for a test that names none.
const SUITE_TIMEOUT: Duration = Duration::from_secs(30);

/// The environment on our kernel: the reference's, with the suite's own time limits scaled up to
/// the harness's `limit`, so that a test slow on the stand-in CPU ends as the harness's timeout
/// and not as the suite's broken verdict.
fn environment_on_ours(limit: Duration) -> Vec<String> {
    let multiplier = limit.as_secs().div_ceil(SUITE_TIMEOUT.as_secs()).max(1);
    ENVIRONMENT
        .iter()
        .map(|variable| (*variable).to_owned())
        .chain([format!("{TIMEOUT_MULTIPLIER}{ASSIGNMENT}{multiplier}")])
        .collect()
}

/// One test: a line of a `runtest` file, its name and the shell command that runs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Test {
    pub group: String,
    pub name: String,
    pub command: String,
}

impl Test {
    /// How results files name it: `group:name`.
    pub fn id(&self) -> String {
        format!("{}:{}", self.group, self.name)
    }

    /// The first process's arguments: the shell running the command, as the suite's runners do.
    pub fn arguments(&self) -> [&str; 3] {
        ["/bin/sh", "-c", &self.command]
    }
}

/// The `runtest` files of the exported image at `tar`, by group.
pub fn groups(tar: &Path) -> BTreeMap<String, String> {
    let file = std::fs::File::open(tar)
        .unwrap_or_else(|error| panic!("opening {}: {error}", tar.display()));
    let mut archive = tar::Archive::new(std::io::BufReader::new(file));
    let mut groups = BTreeMap::new();
    for entry in archive.entries().expect("reading the archive") {
        let mut entry = entry.expect("an archive entry");
        let path = entry.path().expect("a path").to_string_lossy().into_owned();
        if let Some(group) = path.strip_prefix("opt/ltp/runtest/")
            && !group.is_empty()
            && entry.header().entry_type() == tar::EntryType::Regular
        {
            let mut text = String::new();
            entry.read_to_string(&mut text).expect("a runtest file");
            groups.insert(group.to_string(), text);
        }
    }
    groups
}

/// The tests of a group's `runtest` file, in its order.
pub fn parse(group: &str, text: &str) -> Vec<Test> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let (name, command) = line.split_once(char::is_whitespace)?;
            Some(Test {
                group: group.to_string(),
                name: name.to_string(),
                command: command.trim().to_string(),
            })
        })
        .collect()
}

/// The tests a selection names: a group, `group:test`, or a test of the suite's groups by its
/// name; the whole suite when it names none.
pub fn select(groups: &BTreeMap<String, String>, selection: &[String]) -> Vec<Test> {
    let group_tests = |group: &str| {
        let text = groups
            .get(group)
            .unwrap_or_else(|| panic!("no runtest file {group}"));
        parse(group, text)
    };
    let suite = || {
        groups
            .keys()
            .filter(|group| !LEFT_OUT.contains(&group.as_str()))
            .flat_map(|group| group_tests(group))
    };
    if selection.is_empty() {
        return suite().collect();
    }
    let mut tests = Vec::new();
    for item in selection {
        if groups.contains_key(item) {
            tests.extend(group_tests(item));
        } else if let Some((group, name)) = item.split_once(':') {
            let found = group_tests(group)
                .into_iter()
                .find(|test| test.name == name);
            tests.push(found.unwrap_or_else(|| panic!("no test {name} in {group}")));
        } else {
            let found = suite().find(|test| &test.name == item);
            tests.push(found.unwrap_or_else(|| panic!("no test {item} in the suite")));
        }
    }
    tests
}

/// The verdict a test's exit code gives, as the suite's own runners read it: the library ORs
/// TFAIL (1), TBROK (2), TWARN (4) and TCONF (32) into it. A code with other bits is not the
/// library's and is kept as it is.
pub fn verdict(exit_code: i32) -> String {
    match exit_code {
        0 => "pass".into(),
        32 => "skipped".into(),
        // `docker run`'s 128 plus a signal (Linux has 64).
        code if (129..=192).contains(&code) => format!("signal{}", code - 128),
        code if code & !(1 | 2 | 4 | 32) != 0 => format!("exit{code}"),
        code if code & 2 != 0 => "broken".into(),
        code if code & 1 != 0 => "fail".into(),
        code if code & 4 != 0 => "warning".into(),
        code => format!("exit{code}"),
    }
}

/// A line of a results file: the test, its verdict, its exit code (-1 when it had none), how long
/// it took, and on our kernel the warnings it gave (`-` for none).
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub id: String,
    pub verdict: String,
    pub exit_code: i32,
    pub seconds: f64,
    pub warnings: String,
}

impl Line {
    pub fn to_text(&self) -> String {
        format!(
            "{}\t{}\t{}\t{:.1}\t{}\n",
            self.id, self.verdict, self.exit_code, self.seconds, self.warnings
        )
    }

    pub fn from_text(text: &str) -> Option<Line> {
        let fields: Vec<&str> = text.trim_end_matches('\n').split('\t').collect();
        let [id, verdict, exit_code, seconds, warnings] = fields[..] else {
            return None;
        };
        Some(Line {
            id: id.to_string(),
            verdict: verdict.to_string(),
            exit_code: exit_code.parse().ok()?,
            seconds: seconds.parse().ok()?,
            warnings: warnings.to_string(),
        })
    }
}

/// Reads a results file.
pub fn read_results(path: &Path) -> Vec<Line> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()))
        .lines()
        .filter_map(Line::from_text)
        .collect()
}

/// Where a test's output goes: `<logs>/<group>/<name>.log`.
pub fn log_path(logs: &Path, test: &Test) -> PathBuf {
    logs.join(&test.group).join(format!("{}.log", test.name))
}

/// Runs a test in the reference, its output to its log; one that runs past `limit` is killed
/// and reported as a timeout.
pub fn run_reference(test: &Test, limit: Duration, logs: &Path) -> Line {
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "wc-kernel-ltp-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    );
    let log = log_path(logs, test);
    std::fs::create_dir_all(log.parent().expect("a directory")).expect("the log directory");
    let output = std::fs::File::create(&log).expect("the log");
    let mut command = Command::new("docker");
    command
        .args(["run", "--rm", "--platform", "linux/arm64", "--name", &name])
        .args(["--hostname", reference::HOSTNAME]);
    for variable in ENVIRONMENT {
        command.args(["--env", variable]);
    }
    command
        .arg(image_name())
        .args(test.arguments())
        .stdin(Stdio::null())
        .stdout(output.try_clone().expect("the log"))
        .stderr(output);
    let started = Instant::now();
    let mut child = command.spawn().expect("running docker");
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for docker") {
            break Some(status);
        }
        if started.elapsed() > limit {
            let _ = Command::new("docker").args(["kill", &name]).output();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let seconds = started.elapsed().as_secs_f64();
    let (verdict, exit_code) = match status.and_then(|status| status.code()) {
        Some(code) => (verdict(code), code),
        None => ("timeout".to_string(), -1),
    };
    Line {
        id: test.id(),
        verdict,
        exit_code,
        seconds,
        warnings: "-".into(),
    }
}

/// Runs tests on our kernel, `jobs` at a time, each under `seed` in a process of its own forked
/// from one that holds the image's files in a prepared kernel, so the files are loaded once. A
/// test that runs past `limit` is killed and reported as a timeout; one whose kernel ends no turn
/// and takes no backend answer for [`STUCK`], or whose processes all wait with nothing to wake them, is a hang; one whose
/// kernel fails is a crash, and one the stand-in CPU cannot run is unsupported. Each finished
/// test's line goes to `report` as it ends.
pub fn run_ours(
    tests: &[Test],
    tar: &Path,
    seed: u64,
    jobs: usize,
    limit: Duration,
    logs: &Path,
    mut report: impl FnMut(&Line),
) -> Vec<Line> {
    let served = served_image_name();
    let environment = environment_on_ours(limit);
    let environment: Vec<&str> = environment.iter().map(String::as_str).collect();
    let found = Command::new("docker")
        .args(["image", "inspect", &served])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("running docker");
    assert!(found.success(), "no image {served}: run ltp/build.sh");
    let mut prepared = {
        let bytes =
            std::fs::read(tar).unwrap_or_else(|error| panic!("reading {}: {error}", tar.display()));
        let files = image::from_tar(&bytes);
        drop(bytes);
        Some(driver::prepare(&container(
            &files,
            &[],
            &environment,
            &served,
        )))
    };
    let mut lines: Vec<Option<Line>> = vec![None; tests.len()];
    let mut running: BTreeMap<libc::pid_t, (usize, Instant)> = BTreeMap::new();
    let mut next = 0;
    while next < tests.len() || !running.is_empty() {
        while next < tests.len() && running.len() < jobs.max(1) {
            let test = &tests[next];
            let log = log_path(logs, test);
            std::fs::create_dir_all(log.parent().expect("a directory")).expect("the log directory");
            let _ = std::fs::remove_file(result_path(&log));
            // SAFETY: this process has one thread; the child runs the test alone and ends with
            // `_exit`.
            match unsafe { libc::fork() } {
                -1 => panic!("fork failed"),
                0 => run_one(
                    prepared.take().expect("a prepared kernel"),
                    test,
                    seed,
                    &log,
                    &environment,
                    &served,
                ),
                child => {
                    running.insert(child, (next, Instant::now()));
                }
            }
            next += 1;
        }
        let mut status = 0;
        // SAFETY: a plain wait for any child, not waiting.
        let ended = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if ended > 0 {
            if let Some((index, started)) = running.remove(&ended) {
                let line = ended_line(&tests[index], &log_path(logs, &tests[index]), started);
                report(&line);
                lines[index] = Some(line);
            }
            continue;
        }
        let late: Vec<libc::pid_t> = running
            .iter()
            .filter(|(_, (_, started))| started.elapsed() > limit)
            .map(|(pid, _)| *pid)
            .collect();
        for pid in late {
            // SAFETY: the child is ours and has not been reaped.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, &mut status, 0);
            }
            let (index, started) = running.remove(&pid).expect("a running test");
            let line = Line {
                id: tests[index].id(),
                verdict: "timeout".into(),
                exit_code: -1,
                seconds: started.elapsed().as_secs_f64(),
                warnings: "-".into(),
            };
            report(&line);
            lines[index] = Some(line);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    lines
        .into_iter()
        .map(|line| line.expect("every test ran"))
        .collect()
}

/// A test's container: its root on the backend, served from `root_image`, as the reference's
/// root is its image's.
fn container<'a>(
    files: &'a [image::File],
    arguments: &'a [&'a str],
    environment: &'a [&'a str],
    root_image: &'a str,
) -> driver::Container<'a> {
    driver::Container {
        files,
        path: "/bin/sh",
        arguments,
        environment,
        working_directory: "/",
        backend: Some("/"),
        // Opened for the page cache, so files map shared as on the reference's overlay.
        cached: true,
        published: &[],
        root_image: Some(root_image),
        address_space: crate::standin::SPACE_32_BIT,
    }
}

/// How long a run on our kernel may end no turn and take no backend answer before it counts as
/// hung.
const STUCK: Duration = Duration::from_secs(120);

/// Where a test's process leaves its line.
fn result_path(log: &Path) -> PathBuf {
    log.with_extension("result")
}

/// In the forked process: runs the test with its output to its log, leaves its line and ends.
fn run_one(
    mut prepared: driver::Prepared,
    test: &Test,
    seed: u64,
    log: &Path,
    environment: &[&str],
    root_image: &str,
) -> ! {
    prepared.echo = true;
    // The suite's tests run for spans of the clock, as long as the reference's.
    prepared.wall_clock = true;
    let file = std::fs::File::create(log).expect("the log");
    // SAFETY: points this process's standard output and error at the log.
    unsafe {
        use std::os::fd::AsRawFd;
        libc::dup2(file.as_raw_fd(), 1);
        libc::dup2(file.as_raw_fd(), 2);
    }
    let started = Instant::now();
    // A run whose kernel ends no turn and takes no backend answer for this long is stuck in one
    // call or one turn: a hang.
    // One that goes on ending turns past the harness's limit is only slow (a timeout).
    {
        let log = log.to_path_buf();
        let id = test.id();
        std::thread::spawn(move || {
            let mut last = (driver::PROGRESS.load(Ordering::Relaxed), Instant::now());
            loop {
                std::thread::sleep(Duration::from_secs(5));
                let turns = driver::PROGRESS.load(Ordering::Relaxed);
                if turns != last.0 {
                    last = (turns, Instant::now());
                } else if last.1.elapsed() > STUCK {
                    eprintln!("no turn ended and no backend answer came for {STUCK:?}");
                    let line = Line {
                        id: id.clone(),
                        verdict: "hang".into(),
                        exit_code: -1,
                        seconds: started.elapsed().as_secs_f64(),
                        warnings: "-".into(),
                    };
                    let _ = std::fs::write(result_path(&log), line.to_text());
                    // SAFETY: ends this process at once, its run abandoned.
                    unsafe { libc::_exit(0) }
                }
            }
        });
    }
    let arguments = test.arguments();
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        driver::run_prepared(
            prepared,
            &container(&[], &arguments, environment, root_image),
            seed,
            driver::Injections::default(),
        )
    }));
    let seconds = started.elapsed().as_secs_f64();
    let line = match run {
        Ok(Ok(run)) => Line {
            id: test.id(),
            verdict: verdict(run.outcome.exit_code),
            exit_code: run.outcome.exit_code,
            seconds,
            warnings: warnings(&run.warnings),
        },
        Ok(Err(failure)) => {
            eprintln!("no outcome: {failure:?}");
            let mut line = crash_line(test, seconds);
            if matches!(failure, driver::Failure::StandIn(_)) {
                line.verdict = "unsupported".into();
            }
            line
        }
        Err(panic) => {
            let message = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("");
            let mut line = crash_line(test, seconds);
            if message.contains("idle with nothing to wake it") {
                line.verdict = "hang".into();
            }
            line
        }
    };
    std::fs::write(result_path(log), line.to_text()).expect("the result");
    // SAFETY: ends the forked process without running the parent's exit handlers.
    unsafe { libc::_exit(0) }
}

fn crash_line(test: &Test, seconds: f64) -> Line {
    Line {
        id: test.id(),
        verdict: "crash".into(),
        exit_code: -1,
        seconds,
        warnings: "-".into(),
    }
}

/// The line a test's process left, or a crash if it left none.
fn ended_line(test: &Test, log: &Path, started: Instant) -> Line {
    std::fs::read_to_string(result_path(log))
        .ok()
        .and_then(|text| Line::from_text(&text))
        .unwrap_or_else(|| crash_line(test, started.elapsed().as_secs_f64()))
}

/// The kernel's warnings in one field: each reason once, with its details.
fn warnings(warned: &[driver::Warned]) -> String {
    let mut by_reason: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for warning in warned {
        let details = by_reason.entry(&warning.reason).or_default();
        if !details.contains(&warning.detail) {
            details.push(warning.detail);
        }
    }
    if by_reason.is_empty() {
        return "-".into();
    }
    by_reason
        .iter()
        .map(|(reason, details)| {
            let details: Vec<String> = details.iter().map(u64::to_string).collect();
            format!("{reason} ({})", details.join(" "))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// A results line's warnings as (reason, detail) pairs.
fn warned(line: &Line) -> Vec<(String, String)> {
    if line.warnings == "-" {
        return Vec::new();
    }
    line.warnings
        .split("; ")
        .filter_map(|part| {
            let (reason, details) = part.rsplit_once(" (")?;
            let details = details.trim_end_matches(')');
            Some(
                details
                    .split(' ')
                    .map(|detail| (reason.to_string(), detail.to_string()))
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

/// The comparison of two results files: how many verdicts match and every test whose verdict
/// differs, grouped by cause: a crash of the kernel, a hang, a part not built (a warning the
/// test gave that no test whose verdict matches gave), or else a wrong answer.
pub fn compare(reference: &[Line], ours: &[Line]) -> String {
    let theirs: BTreeMap<&str, &Line> = reference
        .iter()
        .map(|line| (line.id.as_str(), line))
        .collect();
    let matched = |line: &&Line| {
        theirs
            .get(line.id.as_str())
            .is_some_and(|their| their.verdict == line.verdict)
    };
    let harmless: std::collections::BTreeSet<(String, String)> =
        ours.iter().filter(matched).flat_map(warned).collect();
    let mut matching = 0;
    let mut by_cause: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut missing = Vec::new();
    let our_ids: std::collections::BTreeSet<&str> =
        ours.iter().map(|line| line.id.as_str()).collect();
    let not_run: Vec<&str> = reference
        .iter()
        .map(|line| line.id.as_str())
        .filter(|id| !our_ids.contains(id))
        .collect();
    for line in ours {
        let Some(their) = theirs.get(line.id.as_str()) else {
            missing.push(line.id.clone());
            continue;
        };
        if their.verdict == line.verdict {
            matching += 1;
            continue;
        }
        let new: Vec<String> = warned(line)
            .into_iter()
            .filter(|pair| !harmless.contains(pair))
            .map(|(reason, detail)| format!("{reason} ({detail})"))
            .collect();
        let cause = match line.verdict.as_str() {
            "crash" => "a crash of the kernel",
            "hang" => "a hang",
            "timeout" => "slower than the time limit (still ending turns)",
            "unsupported" => "an instruction the stand-in CPU does not run",
            _ if !new.is_empty() => "a part not built (a warning no matching test gave)",
            _ => "a wrong answer",
        };
        let mut text = format!(
            "{}: reference {}, ours {}",
            line.id, their.verdict, line.verdict
        );
        if !new.is_empty() {
            text += &format!(" [{}]", new.join("; "));
        }
        by_cause.entry(cause).or_default().push(text);
    }
    let differing: usize = by_cause.values().map(Vec::len).sum();
    let mut text = format!(
        "{} tests compared: {matching} verdicts match, {differing} differ, {} not run\n",
        matching + differing,
        not_run.len()
    );
    for (cause, lines) in &by_cause {
        text += &format!("\n{cause}: {}\n", lines.len());
        for line in lines {
            text += &format!("  {line}\n");
        }
    }
    if !missing.is_empty() {
        text += &format!("\nnot in the reference's results: {}\n", missing.join(" "));
    }
    if !not_run.is_empty() {
        text += &format!("\nnot in our results: {}\n", not_run.join(" "));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: &str, verdict: &str, warnings: &str) -> Line {
        Line {
            id: id.into(),
            verdict: verdict.into(),
            exit_code: 0,
            seconds: 1.0,
            warnings: warnings.into(),
        }
    }

    #[test]
    fn runtest_lines_are_parsed() {
        let text = "# a comment\n\nfirst first_bin -x\n  second\tsh -c 'a b'  \nalone\n";
        let tests = parse("g", text);
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].id(), "g:first");
        assert_eq!(tests[0].command, "first_bin -x");
        assert_eq!(tests[1].command, "sh -c 'a b'");
    }

    #[test]
    fn exit_codes_give_the_library_verdicts() {
        let cases = [
            (0, "pass"),
            (32, "skipped"),
            (1, "fail"),
            (2, "broken"),
            (3, "broken"),
            (34, "broken"),
            (5, "fail"),
            (33, "fail"),
            (4, "warning"),
            (36, "warning"),
            (139, "signal11"),
            (192, "signal64"),
            (255, "exit255"),
            (64, "exit64"),
            (193, "exit193"),
        ];
        for (code, expected) in cases {
            assert_eq!(verdict(code), expected, "exit code {code}");
        }
    }

    #[test]
    fn results_lines_round_trip() {
        let original = Line {
            id: "syscalls:read01".into(),
            verdict: "broken".into(),
            exit_code: 2,
            seconds: 3.5,
            warnings: "proc entry not built (1 2); page residency not tracked (0)".into(),
        };
        let text = original.to_text();
        assert_eq!(Line::from_text(&text), Some(original.clone()));
        assert_eq!(Line::from_text("too\tfew\n"), None);
        assert_eq!(
            warned(&original),
            [
                ("proc entry not built".into(), "1".into()),
                ("proc entry not built".into(), "2".into()),
                ("page residency not tracked".into(), "0".into()),
            ]
        );
        assert!(warned(&line("a", "pass", "-")).is_empty());
    }

    #[test]
    fn comparison_groups_differences_and_lists_tests_missing_on_either_side() {
        let reference = [
            line("g:same", "pass", "-"),
            line("g:wrong", "pass", "-"),
            line("g:gap", "pass", "-"),
            line("g:crashed", "pass", "-"),
            line("g:unrun", "pass", "-"),
        ];
        let ours = [
            line("g:same", "pass", "known (1)"),
            line("g:wrong", "fail", "known (1)"),
            line("g:gap", "broken", "new (7)"),
            line("g:crashed", "crash", "-"),
            line("g:extra", "pass", "-"),
        ];
        let text = compare(&reference, &ours);
        assert!(text.starts_with("4 tests compared: 1 verdicts match, 3 differ, 1 not run\n"));
        assert!(text.contains("a wrong answer: 1\n  g:wrong: reference pass, ours fail\n"));
        assert!(text.contains("  g:gap: reference pass, ours broken [new (7)]\n"));
        assert!(text.contains("a crash of the kernel: 1\n"));
        assert!(text.contains("not in the reference's results: g:extra\n"));
        assert!(text.contains("not in our results: g:unrun\n"));
    }
}
