//! Running one command of the reference image in both runners and comparing everything it did.

use std::collections::BTreeMap;

use crate::image::{self, File, Kind};
use crate::{driver, reference};

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

fn shapes_of_kernel(files: &[wcontainers_kernel::FileRecord]) -> BTreeMap<String, Shape> {
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
fn on_path(command: &str) -> String {
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
        hooks,
        ..driver::prepare(&container)
    };
    driver::run_prepared(prepared, &container, seed, Default::default())
        .unwrap_or_else(|failure| panic!("{command:?}: no outcome on our kernel: {failure:?}"))
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

/// Runs a command of the reference image (the first argument names it) as the container's first
/// process in the reference and on our kernel twice, with the root file system in memory and on
/// the file backend, with `/` as the working directory, and asserts that output, exit status and
/// the files changed are the same each time. Returns our run with the root in memory.
pub fn compare(command: &[&str]) -> driver::Run {
    let base = shapes_of_image(image::reference_image());
    let (theirs, their_files) = reference::run_command(command);
    let their_changes = changes(&base, &shapes_of_image(&their_files));
    let mut runs = [None, Some("/")].map(|backend| {
        let root = if backend.is_some() {
            "root on the backend"
        } else {
            "root in memory"
        };
        let ours = run_ours_on(command, 0, backend);
        let messages: u64 = ours
            .counters
            .iter()
            .filter(|counter| counter.reason == wcontainers_kernel::reason::BACKEND_MESSAGE)
            .map(|counter| counter.count)
            .sum();
        assert_eq!(
            messages > 0,
            backend.is_some(),
            "{command:?}, {root}: {messages} messages to the backend"
        );
        let our_changes = changes(&base, &shapes_of_kernel(&ours.files));
        assert_eq!(
            String::from_utf8_lossy(&ours.outcome.stdout),
            String::from_utf8_lossy(&theirs.stdout),
            "{command:?}, {root}: standard output"
        );
        assert_eq!(
            String::from_utf8_lossy(&ours.outcome.stderr),
            String::from_utf8_lossy(&theirs.stderr),
            "{command:?}, {root}: standard error"
        );
        assert_eq!(
            ours.outcome.exit_code, theirs.exit_code,
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
                    "{path}: ours {}, theirs {}",
                    describe(our_changes.get(path)),
                    describe(their_changes.get(path))
                )
            })
            .collect();
        assert!(
            differing.is_empty(),
            "{command:?}, {root}: files changed differently:\n{}",
            differing.join("\n")
        );
        Some(ours)
    });
    runs[0].take().expect("the run with the root in memory")
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
