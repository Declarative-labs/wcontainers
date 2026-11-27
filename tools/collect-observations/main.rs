//! Measures every instruction on every kind of core of this machine and writes each kind's sample as a baked file.
//!
//! `collect-observations <folder>` writes `<folder>/<kind>.rs`, the labels of the machine as a comment at the top. On
//! a failure it writes the evidence to `<folder>/collection-failure.txt` and exits with a failure status.

use std::path::Path;
use std::process::ExitCode;

const USAGE: &str =
    "usage: collect-observations <folder to write a baked file for each kind of core into>";
#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
)))]
const SUPPORTED_MACHINES: &str = "collect-observations runs on Linux on ARM64 or x86-64, with KVM";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod arm64;
#[cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
mod collection;
#[cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
mod counted_states;
#[cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
mod failure;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod x86_64;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let (Some(output), None) = (arguments.next(), arguments.next()) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    collect_on_this_machine(Path::new(&output))
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn collect_on_this_machine(output: &Path) -> ExitCode {
    arm64::collect(output)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn collect_on_this_machine(output: &Path) -> ExitCode {
    x86_64::collect(output)
}

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
)))]
fn collect_on_this_machine(_: &Path) -> ExitCode {
    eprintln!("{SUPPORTED_MACHINES}");
    ExitCode::FAILURE
}
