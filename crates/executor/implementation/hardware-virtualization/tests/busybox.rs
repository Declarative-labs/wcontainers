//! N3's bar: every command of the kernel test crate's busybox `text` and `files` tests runs as a container's
//! first process in the kernel inside the machine, with the root file system in memory and the translator at
//! look-ahead none, and gives the reference's output, exit code and changed files (ARM64 Linux in Docker).
//!
//! This track's own commands run the same way.
//!
//! `WCONTAINERS_KERNEL_SEED` seeds the policy and the host's random bytes; `WCONTAINERS_KERNEL_SEEDS` runs each
//! command with that many seeds, it and the ones after it.

#[cfg(host_hypervisor)]
#[path = "../../hardware-virtualization-inside/bin/native-kernel/protocol.rs"]
mod protocol;

#[cfg(host_hypervisor)]
mod common;

#[cfg(host_hypervisor)]
mod container;

#[cfg(host_hypervisor)]
fn main() {
    hosted::main();
}

#[cfg(not(host_hypervisor))]
fn main() {
    println!("the busybox test needs a hypervisor this crate drives; nothing to run on this host");
}

#[cfg(host_hypervisor)]
mod hosted {
    use std::path::MAIN_SEPARATOR_STR;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    use wcontainers_executor_hardware_virtualization_inside::container::{
        File, FileKind, Starting,
    };
    use wcontainers_executor_unicorn::busybox::{FILES, Reference, TEXT, each, on_path};
    use wcontainers_executor_unicorn::driver::{Outcome, exit_code};
    use wcontainers_executor_unicorn::image::{self, Kind};
    use wcontainers_executor_unicorn::reference;

    use crate::common::{be_signed, load_average};
    use crate::container::{native, seeds, summary};

    /// This track's commands: a program that zeroes whole blocks.
    const OWN: &[&[&str]] = &[&["sh", "-c", "bzip2 -c /etc/passwd | bzip2 -dc | md5sum"]];

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
        let seeds = seeds();
        let started = Instant::now();
        let image = image::reference_image();
        let files = files(image);
        let commands: Vec<&[&str]> = TEXT.iter().chain(FILES).chain(OWN).copied().collect();
        each(&commands, |command| {
            let reference = Reference::of(command);
            for &seed in &seeds {
                let began = Instant::now();
                // The reference's clock is the machine's: a file's time shown to the minute must match.
                let wall_start = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("a clock after 1970")
                    .as_nanos() as u64;
                let ours = native(seed, wall_start, &files, &starting(command, image));
                println!(
                    "{command:?} seed {seed:#x}: status {:#x} after {} turns, {} exits, {:?}, warned {:?}",
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
                reference.check(command, ROOT_IN_MEMORY, &outcome, &ours.root);
            }
        });
        println!(
            "busybox: {} commands gave the reference's output, exit code and files with each of {} seeds from {:#x}; in {:?}, load {}",
            commands.len(),
            seeds.len(),
            seeds[0],
            started.elapsed(),
            load_average()
        );
    }
}
