//! Every command of the kernel test crate's busybox `text` and `files` tests, and the virtual-machine executor's own,
//! runs as a container's first process in the kernel's loop under form P, with the root file system in memory, and
//! gives the reference's output, exit code and changed files (ARM64 Linux in Docker on this machine).
//!
//! `WCHP_KERNEL_SEED` seeds the policy and the host's random bytes; `WCHP_KERNEL_SEEDS` runs each command with that
//! many seeds, it and the ones after it.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

#[expect(
    dead_code,
    reason = "the policy's turns are compared by the kernel test alone"
)]
mod kernel_host;

use std::sync::{Arc, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use kernel_host::{Program, run_container};
use wcontainers_executor_host_process::executor::HostProcessExecutor;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_unicorn::busybox::{FILES, Reference, TEXT, each, on_path};
use wcontainers_executor_unicorn::driver::{Outcome, exit_code};
use wcontainers_executor_unicorn::{image, reference};
use wcontainers_runtime_single_kernel::Kernel;

const SEED_VARIABLE: &str = "WCHP_KERNEL_SEED";
const SEEDS_VARIABLE: &str = "WCHP_KERNEL_SEEDS";
const DEFAULT_SEED: u64 = 1;

/// The virtual-machine executor's own commands: a program that zeroes whole blocks, through a pipe of two.
const OWN: &[&[&str]] = &[&["sh", "-c", "bzip2 -c /etc/passwd | bzip2 -dc | md5sum"]];

/// The run's name in the check's messages.
const ROOT_IN_MEMORY: &str = "root in memory, form P";

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn seeds() -> Vec<u64> {
    let first = setting(SEED_VARIABLE, DEFAULT_SEED);
    (0..setting(SEEDS_VARIABLE, 1))
        .map(|k| first.wrapping_add(k))
        .collect()
}

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

fn host_process() -> HostProcessExecutor {
    HostProcessExecutor::new(
        HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
            .clone(),
    )
}

/// Runs `command` of the reference image under form P, found on the image's PATH, with the reference's environment,
/// host name and root's groups.
fn ours(
    command: &[&str],
    seed: u64,
    wall_start: u64,
) -> (Outcome, Vec<wcontainers_kernel_linux::FileRecord>) {
    let files = image::reference_image();
    let mut kernel = Kernel::new();
    image::add_files(&mut kernel, files);
    let path = on_path(command[0]);
    let mut arguments: Vec<&[u8]> = vec![path.as_bytes()];
    arguments.extend(command[1..].iter().map(|argument| argument.as_bytes()));
    let environment: Vec<&[u8]> = reference::ENVIRONMENT
        .iter()
        .map(|variable| variable.as_bytes())
        .collect();
    let groups = image::root_groups(files);
    let program = Program {
        path: path.as_bytes(),
        arguments: &arguments,
        environment: &environment,
        hostname: reference::HOSTNAME.as_bytes(),
        groups: &groups,
        limit: None,
    };
    let ran = run_container(&mut host_process(), kernel, seed, wall_start, &program);
    (
        Outcome {
            stdout: ran.output.stdout,
            stderr: ran.output.stderr,
            exit_code: exit_code(ran.status),
        },
        ran.files,
    )
}

#[test]
fn busybox_text_and_files_commands_give_the_reference_s_output_exit_code_and_files() {
    let seeds = seeds();
    let started = Instant::now();
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
            let (outcome, files) = ours(command, seed, wall_start);
            println!(
                "{command:?} seed {seed}: exit code {} in {:?}",
                outcome.exit_code,
                began.elapsed()
            );
            reference.check(command, ROOT_IN_MEMORY, &outcome, &files);
        }
    });
    println!(
        "busybox: {} commands gave the reference's output, exit code and files with each of {} seeds from {} in {:?}",
        commands.len(),
        seeds.len(),
        seeds[0],
        started.elapsed()
    );
}
