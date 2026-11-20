//! Runs wcontainers-kernel on a stand-in CPU and compares it with real ARM64 Linux.

pub mod backend;
pub mod buffer;
pub mod busybox;
pub mod driver;
pub mod generate;
pub mod guest;
pub mod hooks;
pub mod image;
pub mod layer1;
pub mod libctest;
pub mod ltp;
pub mod network;
pub mod reference;
pub mod runtimes;
pub mod standin;
pub mod streams;

use std::path::Path;

/// Runs a program from a host directory on our kernel and in the reference, with the same arguments, in
/// a container of the reference image with the directory at `/wc`, and returns both.
pub fn run_both(program: &Path, arguments: &[&str]) -> (driver::Run, driver::Outcome) {
    let name = reference::in_guest_directory(program.file_name().expect("program has a name"));
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
    run_ours_wall_moved(directory, path, arguments, seed, saves, 0)
}

/// As [`run_ours_saving`], with the host's wall clock moved on `wall_step` nanoseconds against its
/// monotonic clock at each save.
pub fn run_ours_wall_moved(
    directory: &Path,
    path: &str,
    arguments: &[&str],
    seed: u64,
    saves: usize,
    wall_step: u64,
) -> driver::Run {
    let injections = driver::Injections {
        saves,
        wall_step,
        ..Default::default()
    };
    run_ours_injected(directory, path, arguments, seed, injections)
}

/// As [`run_ours`], with the run's seed and what it has done to it.
pub fn run_ours_injected(
    directory: &Path,
    path: &str,
    arguments: &[&str],
    seed: u64,
    injections: driver::Injections,
) -> driver::Run {
    run_ours_on(
        directory,
        path,
        arguments,
        seed,
        injections,
        None,
        &[],
        false,
    )
}

/// As [`run_ours_saving`], with ports of this machine's loopback reaching the container's as
/// (container port, outside port), as `docker run -p` publishes them.
pub fn run_ours_published(
    directory: &Path,
    path: &str,
    arguments: &[&str],
    seed: u64,
    saves: usize,
    published: &[(u16, u16)],
) -> driver::Run {
    let injections = driver::Injections {
        saves,
        ..Default::default()
    };
    run_ours_on(
        directory, path, arguments, seed, injections, None, published, false,
    )
}

/// As [`run_ours_injected`], with the root file system on the backend (`Some("/")`) or in memory,
/// the ports published, and the kernel on the machine's clock or a repeatable one.
#[allow(
    clippy::too_many_arguments,
    reason = "each names how the container runs"
)]
fn run_ours_on(
    directory: &Path,
    path: &str,
    arguments: &[&str],
    seed: u64,
    injections: driver::Injections,
    backend: Option<&str>,
    published: &[(u16, u16)],
    wall_clock: bool,
) -> driver::Run {
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(directory, guest_directory_entry()));
    let container = driver::Container {
        files: &files,
        path,
        arguments,
        environment: reference::ENVIRONMENT,
        working_directory: reference::ROOT_DIRECTORY,
        backend,
        cached: false,
        published,
        root_image: None,
        address_space: standin::SPACE_32_BIT,
    };
    let mut prepared = driver::prepare(&container);
    prepared.wall_clock = wall_clock;
    driver::run_prepared(prepared, &container, seed, injections)
        .unwrap_or_else(|failure| panic!("no outcome on our kernel: {failure:?}"))
}

/// Builds the guest program `name` into `out`, runs it with `arguments` on our kernel and in the
/// reference, and checks that both print the same lines and end the same way; returns our run for its
/// counters.
pub fn assert_same_output(out: &Path, name: &str, arguments: &[&str]) -> driver::Run {
    let program = guest::build(name, &[], out);
    let (ours, theirs) = run_both(&program, arguments);
    assert_same(ours, &theirs)
}

/// As [`assert_same_output`], with our kernel on the machine's clock, as the Linux Test Project's
/// tests run.
pub fn assert_same_output_on_wall_clock(out: &Path, name: &str) -> driver::Run {
    let program = guest::build(name, &[], out);
    let path = reference::in_guest_directory(program.file_name().expect("program has a name"));
    let ours = run_ours_on(
        program.parent().expect("program has a directory"),
        &path,
        &[path.as_str()],
        0,
        driver::Injections::default(),
        None,
        &[],
        true,
    );
    assert_same(ours, &reference::run(&program, &[]))
}

/// Checks that both runs printed the same lines and ended the same way; returns ours.
fn assert_same(ours: driver::Run, theirs: &driver::Outcome) -> driver::Run {
    let ours_text = String::from_utf8_lossy(&ours.outcome.stdout).into_owned();
    let theirs_text = String::from_utf8_lossy(&theirs.stdout).into_owned();
    let differences: Vec<String> = ours_text
        .lines()
        .zip(theirs_text.lines())
        .filter(|(our, their)| our != their)
        .map(|(our, their)| format!("ours {our:?}\ntheirs {their:?}"))
        .collect();
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    assert_eq!(&ours.outcome, theirs);
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
    same_output_seeded(out, name, arguments, saves, reference::run, None)
}

/// As [`assert_same_output_seeded`], with ours on a root file system on the file backend, the
/// image and the program unpacked into it.
pub fn assert_same_output_on_backend_root_seeded(
    out: &Path,
    name: &str,
    arguments: &[&str],
    saves: usize,
) -> Vec<driver::Run> {
    same_output_seeded(
        out,
        name,
        arguments,
        saves,
        reference::run,
        Some(reference::ROOT_DIRECTORY),
    )
}

/// As [`assert_same_output_seeded`], with the reference on one CPU like the made-up machine.
pub fn assert_same_output_one_cpu_seeded(
    out: &Path,
    name: &str,
    arguments: &[&str],
    saves: usize,
) -> Vec<driver::Run> {
    same_output_seeded(out, name, arguments, saves, reference::run_one_cpu, None)
}

fn same_output_seeded(
    out: &Path,
    name: &str,
    arguments: &[&str],
    saves: usize,
    reference: fn(&Path, &[&str]) -> driver::Outcome,
    backend: Option<&str>,
) -> Vec<driver::Run> {
    let program = guest::build(name, &[], out);
    let theirs = reference(&program, arguments);
    let path = reference::in_guest_directory(program.file_name().expect("a name"));
    let mut all = vec![path.as_str()];
    all.extend_from_slice(arguments);
    let directory = program.parent().expect("program has a directory");
    let mut runs = Vec::new();
    for seed in schedule_seeds() {
        let injections = driver::Injections {
            saves,
            ..Default::default()
        };
        let ours = run_ours_on(
            directory,
            &path,
            &all,
            seed,
            injections,
            backend,
            &[],
            false,
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
        runs.push(ours);
    }
    runs
}

/// Runs `command` (the guest program `program` built into `out` and run from `/wc`, or else a
/// command of the image) with the client given, in the reference once and on our kernel under
/// every seed of [`schedule_seeds`]; checks that both give the same, that the kernel kept the credit
/// rules and closed its output ends; returns our runs.
pub fn assert_same_streams(
    out: &Path,
    program: Option<&str>,
    command: &[&str],
    client: &streams::StreamClient,
) -> Vec<driver::Run> {
    assert_same_streams_on(out, program, command, client, None)
}

/// As [`assert_same_streams`], with the file backend mounted at `backend` (`/` for the root).
pub fn assert_same_streams_on(
    out: &Path,
    program: Option<&str>,
    command: &[&str],
    client: &streams::StreamClient,
    backend: Option<&str>,
) -> Vec<driver::Run> {
    let mut all: Vec<String> = command.iter().map(|part| part.to_string()).collect();
    let directory = match program {
        Some(name) => {
            let built = guest::build(name, &[], out);
            all.insert(0, reference::in_guest_directory(name));
            built
                .parent()
                .expect("program has a directory")
                .to_path_buf()
        }
        None => {
            std::fs::create_dir_all(out).expect("creating the output directory");
            out.to_path_buf()
        }
    };
    let all: Vec<&str> = all.iter().map(String::as_str).collect();
    let theirs = reference::run_with_client(&directory, &all, client);
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(&directory, guest_directory_entry()));
    // The image's commands are found on its PATH, as the reference's are.
    let path = match program {
        Some(_) => all[0].to_string(),
        None => image_command(all[0]),
    };
    let environment = streams_environment(client);
    let container = driver::Container {
        files: &files,
        path: &path,
        arguments: &all,
        environment: &environment,
        working_directory: reference::ROOT_DIRECTORY,
        backend,
        cached: false,
        root_image: None,
        published: &[],
        address_space: standin::SPACE_32_BIT,
    };
    let mut runs = Vec::new();
    for seed in schedule_seeds() {
        let ours = driver::run_with_client(
            &container,
            client.clone(),
            seed,
            driver::Injections::default(),
        )
        .unwrap_or_else(|failure| panic!("no outcome on our kernel: {failure:?}"));
        assert_eq!(ours.broken, Vec::<String>::new(), "seed {seed}");
        assert!(ours.outputs_closed, "seed {seed}: an output end left open");
        let show = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        assert_eq!(
            (show(&ours.outcome.stdout), show(&ours.outcome.stderr)),
            (show(&theirs.stdout), show(&theirs.stderr)),
            "seed {seed}"
        );
        assert_eq!(ours.outcome, theirs, "seed {seed}");
        runs.push(ours);
    }
    runs
}

/// The image's entry for [`reference::GUEST_DIRECTORY`] (its name under the root).
pub fn guest_directory_entry() -> &'static str {
    Path::new(reference::GUEST_DIRECTORY)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("the guest directory has a name")
}

/// The path of a command of the image, found on its PATH as the reference finds it.
pub fn image_command(name: &str) -> String {
    Path::new(reference::IMAGE_BINARIES)
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// The environment a container with the client given starts with, as `docker run` gives it.
pub fn streams_environment(client: &streams::StreamClient) -> Vec<&'static str> {
    let mut environment = reference::ENVIRONMENT.to_vec();
    // Docker puts it before HOME, the last.
    if client.terminal.is_some() {
        environment.insert(environment.len() - 1, reference::TERMINAL_VARIABLE);
    }
    environment
}
