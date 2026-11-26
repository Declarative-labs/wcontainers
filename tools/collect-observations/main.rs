//! COLLECT OBSERVATIONS: measures every instruction on every kind of core of this machine, on every state the state
//! generator gives, and writes each kind's sample as a baked Rust file named after the kind, with the labels of the
//! machine as a comment at its top.
//!
//! `collect-observations <folder>` writes `<folder>/<kind>.rs` for each kind of core. When the collection fails it
//! writes what the core did to `<folder>/collection-failure.txt`, says in one line what failed, and exits with a
//! failure status. It runs on Linux on ARM64 or x86-64 with KVM, and opens no network connection.

use std::path::Path;
use std::process::ExitCode;

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
        eprintln!(
            "usage: collect-observations <folder to write a baked file for each kind of core into>"
        );
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
    eprintln!("collect-observations runs on Linux on ARM64 or x86-64, with KVM");
    ExitCode::FAILURE
}
