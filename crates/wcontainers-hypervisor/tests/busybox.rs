//! N3's bar: every command of the kernel test crate's busybox `text` and `files` tests runs as a container's
//! first process in the kernel inside the machine, with the root file system in memory and the translator at
//! look-ahead none, and gives the reference's output, exit code and changed files (ARM64 Linux in Docker).
//!
//! `WCONTAINERS_KERNEL_SEED` seeds the policy and the host's random bytes.

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-kernel/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
mod container;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the busybox test needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::MAIN_SEPARATOR_STR;
    use std::time::Instant;

    use wcontainers_inside::container::{File, FileKind, Starting};
    use wcontainers_kernel_test::busybox::{FILES, Reference, TEXT, each, on_path};
    use wcontainers_kernel_test::driver::{Outcome, exit_code};
    use wcontainers_kernel_test::image::{self, Kind};
    use wcontainers_kernel_test::reference;

    use crate::common::{be_signed, load_average, setting};
    use crate::container::{DEFAULT_SEED, SEED_VARIABLE, native, summary};

    /// The run's name in the check's messages.
    const ROOT_IN_MEMORY: &str = "root in memory, native";

    /// The reference image's files as a container starts with them, as the inside's kernel takes them.
    fn files(image: &[image::File]) -> Vec<File> {
        image::as_started(image)
            .map(|file| File {
                path: format!("{MAIN_SEPARATOR_STR}{}", file.path).into_bytes(),
                mode: file.mode,
                uid: file.uid,
                gid: file.gid,
                mtime_seconds: file.mtime,
                kind: match file.kind {
                    Kind::Directory => FileKind::Directory,
                    Kind::Regular(contents) => FileKind::Regular(contents),
                    Kind::Symlink(target) => FileKind::Symlink(target),
                    Kind::HardLink(target) => FileKind::HardLink(target.into_bytes()),
                    Kind::CharacterDevice(major, minor) => {
                        FileKind::CharacterDevice { major, minor }
                    }
                    Kind::BlockDevice(major, minor) => FileKind::BlockDevice { major, minor },
                    Kind::Fifo => FileKind::Fifo,
                },
            })
            .collect()
    }

    /// A command of the image started as Docker starts it: found on the image's PATH, with the reference's
    /// environment, host name and root's groups, at the root.
    fn starting(command: &[&str], image: &[image::File]) -> Starting {
        let bytes = |texts: &[&str]| texts.iter().map(|text| text.as_bytes().to_vec()).collect();
        Starting {
            path: on_path(command[0]).into_bytes(),
            arguments: bytes(command),
            environment: bytes(reference::ENVIRONMENT),
            working_directory: MAIN_SEPARATOR_STR.as_bytes().to_vec(),
            hostname: reference::HOSTNAME.as_bytes().to_vec(),
            groups: image::root_groups(image),
            backend: None,
        }
    }

    pub fn main() {
        be_signed();
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let started = Instant::now();
        let image = image::reference_image();
        let files = files(image);
        let commands: Vec<&[&str]> = TEXT.iter().chain(FILES).copied().collect();
        each(&commands, |command| {
            let theirs = Reference::of(command);
            let began = Instant::now();
            let ours = native(seed, &files, &starting(command, image));
            println!(
                "{command:?}: status {:#x} after {} turns, {} exits, {:?}, warned {:?}",
                ours.status,
                ours.turns,
                ours.exits,
                began.elapsed(),
                summary(&ours.warnings)
            );
            let outcome = Outcome {
                stdout: ours.host.stdout,
                stderr: ours.host.stderr,
                exit_code: exit_code(ours.status),
            };
            theirs.check(command, ROOT_IN_MEMORY, &outcome, &ours.root);
        });
        println!(
            "busybox: {} commands gave the reference's output, exit code and files; seed {seed:#x}; in {:?}, load {}",
            commands.len(),
            started.elapsed(),
            load_average()
        );
    }
}
