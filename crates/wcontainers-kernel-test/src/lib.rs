//! Runs wcontainers-kernel on a stand-in CPU and compares it with real ARM64 Linux.

pub mod driver;
pub mod generate;
pub mod guest;
pub mod reference;
pub mod standin;

use std::path::Path;

/// Runs a program on our kernel and in the reference with the same arguments, and returns both.
pub fn run_both(program: &Path, arguments: &[&str]) -> (driver::Run, driver::Outcome) {
    let bytes = std::fs::read(program).expect("reading the program");
    let name = format!(
        "/wc/{}",
        program
            .file_name()
            .expect("program has a name")
            .to_string_lossy()
    );
    let mut all = vec![name.as_str()];
    all.extend_from_slice(arguments);
    let ours = driver::run(&bytes, &all, reference::ENVIRONMENT, 0).expect("starting the program");
    let theirs = reference::run(program, arguments);
    (ours, theirs)
}
