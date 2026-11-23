//! Running one command of the reference image in both runners and comparing everything it did.

use std::collections::BTreeMap;

use crate::image::{self, File, Kind};
use crate::{driver, reference};

/// Busybox's applet that removes files, which shares its name with Docker's command.
pub const RM_APPLET: &str = "rm";

/// Busybox's `dd` applet.
pub const DD_APPLET: &str = "dd";

/// What a file is, for comparing: type and permission bits, owner, group, and contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub contents: Vec<u8>,
}

fn type_bits(kind: &Kind) -> u32 {
    match kind {
        Kind::Directory => 0o040000,
        Kind::Regular(_) | Kind::HardLink(_) => 0o100000,
        Kind::Symlink(_) => 0o120000,
        Kind::CharacterDevice(..) => 0o020000,
        Kind::BlockDevice(..) => 0o060000,
        Kind::Fifo => 0o010000,
    }
}

/// The files of an image by path from the root, with hard links given their target's contents.
pub fn shapes_of_image(files: &[File]) -> BTreeMap<String, Shape> {
    let mut out: BTreeMap<String, Shape> = BTreeMap::new();
    for file in files {
        let contents = match &file.kind {
            Kind::Regular(bytes) | Kind::Symlink(bytes) => bytes.clone(),
            Kind::HardLink(target) => out
                .get(&format!("/{target}"))
                .map(|shape| shape.contents.clone())
                .unwrap_or_default(),
            Kind::CharacterDevice(major, minor) | Kind::BlockDevice(major, minor) => {
                format!("{major},{minor}").into_bytes()
            }
            Kind::Directory | Kind::Fifo => Vec::new(),
        };
        out.insert(
            format!("/{}", file.path),
            Shape {
                mode: type_bits(&file.kind) | file.mode,
                uid: file.uid,
                gid: file.gid,
                contents,
            },
        );
    }
    out
}

pub fn shapes_of_kernel(files: &[wcontainers_kernel_linux::FileRecord]) -> BTreeMap<String, Shape> {
    files
        .iter()
        .map(|file| {
            let contents = if file.mode & 0o170000 == 0o020000 || file.mode & 0o170000 == 0o060000 {
                let (major, minor) = (
                    ((file.rdev >> 8) & 0xfff) as u32,
                    ((file.rdev & 0xff) | ((file.rdev >> 12) & 0xfff00)) as u32,
                );
                format!("{major},{minor}").into_bytes()
            } else {
                file.contents.clone()
            };
            (
                String::from_utf8_lossy(&file.path).into_owned(),
                Shape {
                    mode: file.mode,
                    uid: file.uid,
                    gid: file.gid,
                    contents,
                },
            )
        })
        .collect()
}

/// Paths never compared: what Docker makes or mounts, and the test programs' directory.
fn compared(path: &str) -> bool {
    let skipped_trees = ["/proc", "/dev", "/sys", "/wc"];
    let docker_files = image::DOCKER_FILES
        .iter()
        .any(|file| path == format!("/{file}"));
    !docker_files
        && !skipped_trees
            .iter()
            .any(|tree| path == *tree || path.starts_with(&format!("{tree}/")))
}

/// The paths whose shape differs from the image, with the shape after (None when removed).
pub fn changes(
    base: &BTreeMap<String, Shape>,
    after: &BTreeMap<String, Shape>,
) -> BTreeMap<String, Option<Shape>> {
    let mut out = BTreeMap::new();
    for (path, shape) in after {
        if compared(path) && base.get(path) != Some(shape) {
            out.insert(path.clone(), Some(shape.clone()));
        }
    }
    for path in base.keys() {
        if compared(path) && !after.contains_key(path) {
            out.insert(path.clone(), None);
        }
    }
    out
}

/// Where Docker finds a command: the first directory of the image's PATH that has it.
pub fn on_path(command: &str) -> String {
    [
        "/usr/local/sbin",
        "/usr/local/bin",
        "/usr/sbin",
        "/usr/bin",
        "/sbin",
        "/bin",
    ]
    .iter()
    .map(|directory| format!("{directory}/{command}"))
    .find(|path| {
        image::reference_image()
            .iter()
            .any(|file| format!("/{}", file.path) == *path)
    })
    .unwrap_or_else(|| panic!("{command} is not on the image's PATH"))
}

/// Runs a command of the reference image on our kernel, found on the image's PATH as Docker finds it.
pub fn run_ours(command: &[&str]) -> driver::Run {
    run_ours_seeded(command, 0)
}

/// As [`run_ours`], with the seed the run's choices come from.
pub fn run_ours_seeded(command: &[&str], seed: u64) -> driver::Run {
    run_ours_on(command, seed, None)
}

/// As [`run_ours_seeded`], with what the test does at the kernel's hooks.
pub fn run_ours_hooked(
    command: &[&str],
    seed: u64,
    hooks: Box<dyn crate::hooks::Hooks>,
) -> driver::Run {
    let path = on_path(command[0]);
    let container = container(command, &path, None);
    let prepared = driver::Prepared {
        hooks: Some(hooks),
        ..driver::prepare(&container)
    };
    driver::run_prepared(prepared, &container, seed, Default::default())
        .unwrap_or_else(|failure| panic!("{command:?}: no outcome on our kernel: {failure:?}"))
}

/// As [`run_ours`], with what the container's configuration sets beyond its program.
pub fn run_ours_with(
    command: &[&str],
    options: wcontainers_kernel_linux::StartOptions<'static>,
) -> driver::Run {
    try_ours_with(command, options)
        .unwrap_or_else(|failure| panic!("{command:?}: no outcome on our kernel: {failure:?}"))
}

/// As [`run_ours`], with the kernel's start changed by `prepare`.
pub fn run_ours_prepared(
    command: &[&str],
    prepare: impl FnOnce(&mut driver::Prepared),
) -> driver::Run {
    let path = on_path(command[0]);
    let container = container(command, &path, None);
    let mut prepared = driver::prepare(&container);
    prepare(&mut prepared);
    driver::run_prepared(prepared, &container, 0, Default::default())
        .unwrap_or_else(|failure| panic!("{command:?}: no outcome on our kernel: {failure:?}"))
}

/// As [`run_ours_with`], with a start that fails given back.
pub fn try_ours_with(
    command: &[&str],
    options: wcontainers_kernel_linux::StartOptions<'static>,
) -> Result<driver::Run, driver::Failure> {
    let path = on_path(command[0]);
    let container = container(command, &path, None);
    let prepared = driver::Prepared {
        groups: image::user_groups(container.files, options.user.0, options.group),
        options,
        ..driver::prepare(&container)
    };
    driver::run_prepared(prepared, &container, 0, Default::default())
}

/// A container running a command of the reference image found at `path`.
fn container<'a>(
    command: &'a [&'a str],
    path: &'a str,
    backend: Option<&'a str>,
) -> driver::Container<'a> {
    driver::Container {
        files: image::reference_image(),
        path,
        arguments: command,
        environment: reference::ENVIRONMENT,
        working_directory: reference::ROOT_DIRECTORY,
        backend,
        cached: false,
        published: &[],
        root_image: None,
        address_space: crate::standin::SPACE_32_BIT,
    }
}

/// As [`run_ours_seeded`], with the root file system on the backend (`Some("/")`) or in memory.
fn run_ours_on(command: &[&str], seed: u64, backend: Option<&str>) -> driver::Run {
    let path = on_path(command[0]);
    let container = container(command, &path, backend);
    driver::run(&container, seed)
        .unwrap_or_else(|failure| panic!("{command:?}: no outcome on our kernel: {failure:?}"))
}

/// As [`run_ours_on`], keeping a record of what the host gave the kernel, then again on a fresh
/// kernel and stand-in from that record alone, with another seed and no backend: the outcome, the
/// files left with the root in memory and every guest memory, as each was dropped and at the end,
/// must be the same. Gives the first run.
#[cfg(not(feature = "runtime"))]
pub fn run_ours_replayed(command: &[&str], seed: u64, backend: Option<&str>) -> driver::Run {
    let path = on_path(command[0]);
    let container = container(command, &path, backend);
    let taped = |seed, tape| {
        let prepared = driver::Prepared {
            tape: Some(tape),
            ..driver::prepare(&container)
        };
        driver::run_prepared(prepared, &container, seed, Default::default())
            .unwrap_or_else(|failure| panic!("{command:?}: no outcome on our kernel: {failure:?}"))
    };
    let mut recorded = taped(seed, crate::replay::Tape::recording());
    let tape = recorded.tape.take().expect("a record");
    let replayed = taped(!seed, tape.replaying());
    assert_eq!(
        replayed.outcome, recorded.outcome,
        "{command:?}: the replay's outcome"
    );
    assert!(!recorded.memories.is_empty(), "{command:?}: no memory seen");
    assert_eq!(
        replayed.memories, recorded.memories,
        "{command:?}: the replay's guest memories"
    );
    if backend.is_none() {
        assert_eq!(
            replayed.files, recorded.files,
            "{command:?}: the replay's files"
        );
    }
    recorded
}

/// Runs a command of the reference image (the first argument names it) as the container's first
/// process in the reference and on our kernel twice, with the root file system in memory and on
/// the file backend, with `/` as the working directory, and asserts that output, exit status and
/// the files changed are the same each time. Returns our run with the root in memory.
pub fn compare(command: &[&str]) -> driver::Run {
    let theirs = Reference::of(command);
    let mut runs = [None, Some("/")].map(|backend| {
        let root = if backend.is_some() {
            "root on the backend"
        } else {
            "root in memory"
        };
        #[cfg(not(feature = "runtime"))]
        let ours = run_ours_replayed(command, 0, backend);
        #[cfg(feature = "runtime")]
        let ours = run_ours_on(command, 0, backend);
        let messages: u64 = ours
            .counters
            .iter()
            .filter(|counter| counter.reason == wcontainers_kernel_linux::reason::BACKEND_MESSAGE)
            .map(|counter| counter.count)
            .sum();
        assert_eq!(
            messages > 0,
            backend.is_some(),
            "{command:?}, {root}: {messages} messages to the backend"
        );
        theirs.check(command, root, &ours.outcome, &ours.files);
        Some(ours)
    });
    runs[0].take().expect("the run with the root in memory")
}

/// What a command did in the reference: its outcome and the files it changed.
pub struct Reference {
    pub outcome: driver::Outcome,
    pub changes: BTreeMap<String, Option<Shape>>,
}

impl Reference {
    /// Runs `command` in the reference.
    pub fn of(command: &[&str]) -> Reference {
        let (outcome, files) = reference::run_command(command);
        let changes = changes(
            &shapes_of_image(image::reference_image()),
            &shapes_of_image(&files),
        );
        Reference { outcome, changes }
    }

    /// Asserts that a run of `command` on our kernel (named `root` in messages) gave the reference's output
    /// and exit code and left `files` changed as the reference did.
    pub fn check(
        &self,
        command: &[&str],
        root: &str,
        ours: &driver::Outcome,
        files: &[wcontainers_kernel_linux::FileRecord],
    ) {
        let theirs = &self.outcome;
        let their_changes = &self.changes;
        let our_changes = changes(
            &shapes_of_image(image::reference_image()),
            &shapes_of_kernel(files),
        );
        assert_eq!(
            String::from_utf8_lossy(&ours.stdout),
            String::from_utf8_lossy(&theirs.stdout),
            "{command:?}, {root}: standard output"
        );
        assert_eq!(
            String::from_utf8_lossy(&ours.stderr),
            String::from_utf8_lossy(&theirs.stderr),
            "{command:?}, {root}: standard error"
        );
        assert_eq!(
            ours.exit_code, theirs.exit_code,
            "{command:?}, {root}: exit code"
        );
        let differing: Vec<String> = our_changes
            .keys()
            .chain(their_changes.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|path| our_changes.get(*path) != their_changes.get(*path))
            .map(|path| {
                format!(
                    "\n{path}: ours {}, theirs {}",
                    describe(our_changes.get(path)),
                    describe(their_changes.get(path))
                )
            })
            .collect();
        assert!(
            differing.is_empty(),
            "{command:?}, {root}: files changed differently:{}",
            differing.concat()
        );
    }
}

fn describe(change: Option<&Option<Shape>>) -> String {
    match change {
        None => "unchanged".to_string(),
        Some(None) => "removed".to_string(),
        Some(Some(shape)) => {
            let text = String::from_utf8_lossy(&shape.contents);
            let text: String = text.chars().take(80).collect();
            format!(
                "{:o} {}:{} {:?} ({} bytes)",
                shape.mode,
                shape.uid,
                shape.gid,
                text,
                shape.contents.len()
            )
        }
    }
}

/// Runs `run` on each command, catching what fails, and asserts that none did, reporting every one that did.
pub fn each(commands: &[&[&str]], run: impl Fn(&[&str])) {
    let mut failed = Vec::new();
    for command in commands {
        // Each command's run is its own: what one leaves behind after a panic is not looked at again.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(command)));
        if let Err(error) = result {
            let message = error
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| error.downcast_ref::<&str>().map(|text| text.to_string()))
                .unwrap_or_default();
            failed.push(format!("\n\n{command:?}: {message}"));
        }
    }
    assert!(
        failed.is_empty(),
        "{} of {} differ:{}",
        failed.len(),
        commands.len(),
        failed.concat()
    );
}

/// Commands that read and write text.
pub const TEXT: &[&[&str]] = &[
    &["echo", "hello", "world"],
    &["printf", "%s-%d\\n", "a", "42"],
    &["cat", "/etc/passwd"],
    &["cat", "/nonexistent"],
    &["cat", "/etc/hostname/"],
    &["cat", "/dev/null/"],
    &["head", "-n", "3", "/etc/group"],
    &["tail", "-c", "20", "/etc/os-release"],
    &["wc", "/etc/passwd"],
    &["sort", "-r", "/etc/shells"],
    &["grep", "-n", "root", "/etc/passwd"],
    &["grep", "nothing-matches", "/etc/passwd"],
    &["sed", "-n", "2,3p", "/etc/passwd"],
    &["awk", "-F:", "{print $1}", "/etc/passwd"],
    &["cut", "-d:", "-f1,3", "/etc/group"],
    &["tr", "a-z", "A-Z"],
    &["seq", "1", "5"],
    &["md5sum", "/bin/busybox"],
    &["sha256sum", "/etc/passwd"],
    &["base64", "/etc/hostname"],
    &["od", "-An", "-tx1", "-N", "16", "/bin/busybox"],
    &["expr", "6", "*", "7"],
    &["basename", "/a/b/c.txt", ".txt"],
    &["true"],
    &["false"],
];

/// Commands that make, change and remove files.
pub const FILES: &[&[&str]] = &[
    &["ls", "-R", "/etc/apk"],
    &["mkdir", "-p", "/tmp/a/b/c"],
    &["mkdir", "/etc/passwd"],
    &["touch", "-d", "2020-01-02 03:04:05", "/tmp/new"],
    &[RM_APPLET, "/etc/motd"],
    &[RM_APPLET, "-r", "/etc/apk"],
    &["rmdir", "/home"],
    &["mv", "/etc/motd", "/tmp/motd"],
    &["cp", "-a", "/etc/apk", "/tmp/apk"],
    &[
        "stat",
        "-c",
        "%n %s %f %u %g %h",
        "/etc/passwd",
        "/bin/sh",
        "/dev/shm",
        wcontainers_kernel_linux::NULL_PATH,
    ],
    &[
        "ls",
        "-la",
        "/dev/shm",
        wcontainers_kernel_linux::DEVPTS_PATH,
    ],
    &["ln", "-s", "/etc/passwd", "/tmp/link"],
    &["ln", "/etc/passwd", "/tmp/hard"],
    &["chmod", "4755", "/etc/motd"],
    &["chown", "1:2", "/etc/motd"],
    &["readlink", "-f", "/bin/sh"],
    &["realpath", "/usr/../etc/./passwd"],
    &[
        DD_APPLET,
        "if=/dev/zero",
        "of=/tmp/zero",
        "bs=1024",
        "count=8",
        "status=noxfer",
    ],
    &["tee", "/tmp/out"],
    &["truncate", "-s", "10000", "/tmp/sparse"],
    &["mknod", "/tmp/null", "c", "1", "3"],
    &["mkfifo", "/tmp/fifo"],
    &["test", "-w", "/etc/passwd"],
    &["pwd"],
    &["fallocate", "-l", "8192", "/tmp/f"],
    &["install", "-m", "755", "/etc/passwd", "/tmp/p"],
    &["readahead", "/etc/passwd"],
    &["fsync", "/etc/passwd"],
    &["cmp", "/bin/busybox", "/bin/sh"],
    &["xxd", "-l", "16", "/bin/busybox"],
];
