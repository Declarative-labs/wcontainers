//! The reference: the same program in a default Docker container on real ARM64 Linux.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::driver::Outcome;

/// The image the reference runs in. Static test programs need nothing from it.
pub const IMAGE: &str = "alpine:3.22";

/// The container's host name, set so that both runners see the same environment.
pub const HOSTNAME: &str = "wc-kernel";

/// The environment a default Docker container gives its first process.
pub const ENVIRONMENT: &[&str] = &[
    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    "HOSTNAME=wc-kernel",
    "HOME=/root",
];

/// Runs `program` as the first process of a default container, with `arguments` after its name. The
/// program's directory is mounted read-only at `/wc`.
pub fn run(program: &Path, arguments: &[&str]) -> Outcome {
    run_with(program, arguments, &[])
}

/// As [`run`], on one CPU like the made-up machine, for behaviour that depends on who runs while
/// another waits.
pub fn run_one_cpu(program: &Path, arguments: &[&str]) -> Outcome {
    run_with(program, arguments, &["--cpuset-cpus=0"])
}

fn run_with(program: &Path, arguments: &[&str], options: &[&str]) -> Outcome {
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "wc-kernel-reference-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    );
    let directory = program
        .parent()
        .expect("program has a directory")
        .canonicalize()
        .expect("program directory exists");
    let file = program.file_name().expect("program has a name");
    let output = Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "--name", &name])
        .args(["--hostname", HOSTNAME])
        .args(options)
        .arg("--volume")
        .arg(format!("{}:/wc:ro", directory.display()))
        .arg(IMAGE)
        .arg(Path::new("/wc").join(file))
        .args(arguments)
        .output()
        .expect("running docker");
    let exit_code = output.status.code().expect("docker exited normally");
    assert!(
        !(125..=127).contains(&exit_code),
        "docker could not run the program (exit {exit_code}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Outcome {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code,
    }
}

/// Runs a command of the image (by its name in the image's PATH) as the first process of a default
/// container, and returns what it did and the container's files afterwards, as `docker export` gives
/// them.
pub fn run_command(command: &[&str]) -> (Outcome, Vec<crate::image::File>) {
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "wc-kernel-command-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    );
    let output = Command::new("docker")
        .args(["run", "--platform", "linux/arm64", "--name", &name])
        .args(["--hostname", HOSTNAME])
        .arg(IMAGE)
        .args(command)
        .output()
        .expect("running docker");
    let exported = Command::new("docker")
        .args(["export", &name])
        .output()
        .expect("running docker export");
    let _ = Command::new("docker").args(["rm", "-f", &name]).output();
    let exit_code = output.status.code().expect("docker exited normally");
    assert!(
        !(125..=127).contains(&exit_code),
        "docker could not run the command (exit {exit_code}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    (
        Outcome {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code,
        },
        crate::image::from_tar(&exported.stdout),
    )
}
