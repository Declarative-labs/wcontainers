//! Larger runtimes (layer 2): programs on the language runtimes of unmodified images, pinned by
//! digest. Each runs in the reference as the first process of a container of its image with a
//! host directory at `/wc`, and on our kernel with the image's files and that directory on the
//! file backend (opening files for the page cache), on a host with 64 bit memories, under several schedules with saves at moments
//! drawn from each.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::driver::{self, Outcome};
use crate::image::{self, File};
use crate::reference::HOSTNAME;
use crate::standin::SPACE_64_BIT;

/// CPython 3.13 on Alpine (musl).
pub const PYTHON: &str =
    "python:3.13-alpine@sha256:9ef1a599f863231c3d91e941389bfdc4a2d6070557243d54df1a4e57015b960e";
/// Node 22 on Alpine.
pub const NODE: &str =
    "node:22-alpine@sha256:1314fb30ec37592d5845e5a160d522c0e680972aa534549940b8d73bde88481a";
/// The Eclipse Temurin Java 21 runtime on Alpine.
pub const JAVA: &str = "eclipse-temurin:21-jre-alpine@sha256:ac05c3c42b7c2f1a52c9bd52f01041fa1958231b15ebeae7420429cbc5baf474";
/// The Eclipse Temurin Java 21 development kit, which compiles the Java program.
pub const JDK: &str = "eclipse-temurin:21-jdk-alpine@sha256:82801f9dcdcbad5a05082aba8c86d1da5b72ccb498e71318b1f91b5a477887b3";
/// Go 1.25, which builds the Go program; the program runs on the reference image.
pub const GO: &str =
    "golang:1.25-alpine@sha256:1ae0735f00daffa3aaf1363a5184c0d2dc55c78e3db4ec70241cdac97bf84b59";

/// Where a runtime's programs are kept in this crate.
pub fn programs(runtime: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("runtimes")
        .join(runtime)
}

/// An image's files, as `docker export` of a container made from it gives them. Read once.
pub fn files(image: &str) -> &'static [File] {
    static FILES: OnceLock<Mutex<BTreeMap<String, &'static [File]>>> = OnceLock::new();
    let mut held = FILES
        .get_or_init(Default::default)
        .lock()
        .expect("the files");
    if let Some(files) = held.get(image) {
        return files;
    }
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "wc-kernel-runtime-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    );
    let created = docker(&[
        "create",
        "--platform",
        "linux/arm64",
        "--name",
        &name,
        image,
    ]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let exported = docker(&["export", &name]);
    let _ = docker(&["rm", "-f", &name]);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let files: &'static [File] = image::from_tar(&exported.stdout).leak();
    held.insert(image.to_string(), files);
    files
}

fn docker(arguments: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(arguments)
        .output()
        .expect("running docker")
}

/// One field of an image's configuration, by Go template.
fn config(image: &str, template: &str) -> String {
    let output = docker(&["image", "inspect", "-f", template, image]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("text")
}

/// The environment Docker gives a container's first process: the image's `PATH`, the host
/// name, the image's other variables, and `HOME` unless the image sets it.
pub fn environment(image: &str) -> Vec<String> {
    let image_variables: Vec<String> = config(image, "{{range .Config.Env}}{{println .}}{{end}}")
        .lines()
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    let path = image_variables
        .iter()
        .find(|variable| variable.starts_with("PATH="))
        .cloned()
        .unwrap_or_else(|| {
            "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()
        });
    let mut variables = vec![path, format!("HOSTNAME={HOSTNAME}")];
    variables.extend(
        image_variables
            .iter()
            .filter(|variable| !variable.starts_with("PATH="))
            .cloned(),
    );
    if !variables
        .iter()
        .any(|variable| variable.starts_with("HOME="))
    {
        variables.push("HOME=/root".to_string());
    }
    variables
}

/// The image's working directory, `/` when it names none.
pub fn working_directory(image: &str) -> String {
    let directory = config(image, "{{.Config.WorkingDir}}").trim().to_string();
    if directory.is_empty() {
        "/".to_string()
    } else {
        directory
    }
}

/// Runs `command` as the first process of a default container of `image`, with `directory` at
/// `/wc` read-only.
pub fn reference(image: &str, directory: &Path, command: &[&str]) -> Outcome {
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "wc-kernel-runtime-reference-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    );
    let directory = directory.canonicalize().expect("the directory exists");
    let output = Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "--name", &name])
        .args(["--hostname", HOSTNAME])
        .arg("--volume")
        .arg(format!("{}:/wc:ro", directory.display()))
        .arg(image)
        .args(command)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("running docker");
    let exit_code = output.status.code().expect("docker exited normally");
    assert!(
        !(125..=127).contains(&exit_code),
        "docker could not run the command (exit {exit_code}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Outcome {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code,
    }
}

/// How a run is saved and brought back: how many times, and the most turns between two saves
/// (each test chooses it from its run's length, so the saves fall all through the run).
#[derive(Clone, Copy, Debug)]
pub struct Saves {
    pub count: usize,
    pub gap: u64,
}

/// Runs `command` on our kernel in a container of `image` with `directory` at `/wc`, the root on
/// the file backend, under `seed` with `saves`.
pub fn ours(
    image: &str,
    directory: &Path,
    command: &[&str],
    seed: u64,
    saves: Saves,
) -> driver::Run {
    let mut container_files = files(image).to_vec();
    container_files.extend(image::host_directory(directory, "wc"));
    let environment = environment(image);
    let environment: Vec<&str> = environment.iter().map(String::as_str).collect();
    let working_directory = working_directory(image);
    let path = program_path(&container_files, &environment, command[0]);
    let container = driver::Container {
        files: &container_files,
        path: &path,
        arguments: command,
        environment: &environment,
        working_directory: &working_directory,
        backend: Some("/"),
        // Opened for the page cache, so files map shared as on the reference's overlay.
        cached: true,
        published: &[],
        root_image: None,
        address_space: SPACE_64_BIT,
    };
    let injections = driver::Injections {
        saves: saves.count,
        save_gap: saves.gap,
        ..Default::default()
    };
    driver::run_injected(&container, seed, injections)
        .unwrap_or_else(|failure| panic!("no outcome on our kernel: {failure:?}"))
}

/// Where Docker finds a command named without a directory: the first of the `PATH`'s
/// directories that has it.
fn program_path(files: &[File], environment: &[&str], name: &str) -> String {
    if name.contains('/') {
        return name.to_string();
    }
    let path = environment
        .iter()
        .find_map(|variable| variable.strip_prefix("PATH="))
        .unwrap_or_default();
    path.split(':')
        .map(|directory| format!("{}/{name}", directory.trim_end_matches('/')))
        .find(|candidate| {
            let inside = candidate.trim_start_matches('/');
            files.iter().any(|file| file.path == inside)
        })
        .unwrap_or_else(|| panic!("no {name} on the image's PATH"))
}

/// Runs `command` in the reference once and on our kernel under every seed of
/// [`crate::schedule_seeds`] with `saves` each, checks that every run printed the same, ended the
/// same way and was saved as often as asked, and returns our runs.
pub fn assert_same(
    image: &str,
    directory: &Path,
    command: &[&str],
    saves: Saves,
) -> Vec<driver::Run> {
    let theirs = reference(image, directory, command);
    let mut runs = Vec::new();
    for seed in crate::schedule_seeds() {
        let started = std::time::Instant::now();
        let ours = ours(image, directory, command, seed, saves);
        eprintln!(
            "{} seed {seed}: {:.1} s, {} turns, saved {} times",
            command.join(" "),
            started.elapsed().as_secs_f64(),
            ours.turns,
            ours.saved
        );
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
        // Every save asked for happened: a run shorter than its saves' moments would drop them.
        assert_eq!(ours.saved, saves.count, "seed {seed}: saves made");
        runs.push(ours);
    }
    runs
}

/// The warnings of runs, each reason with its details, as the status file lists what a runtime
/// asked for that is not built.
pub fn warned(runs: &[driver::Run]) -> BTreeMap<String, Vec<u64>> {
    let mut warned: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for warning in runs.iter().flat_map(|run| &run.warnings) {
        let details = warned.entry(warning.reason.clone()).or_default();
        if !details.contains(&warning.detail) {
            details.push(warning.detail);
            details.sort();
        }
    }
    warned
}
