//! Runs wcontainers-kernel on a stand-in CPU and compares it with real ARM64 Linux.

pub mod buffer;
pub mod busybox;
pub mod driver;
pub mod generate;
pub mod guest;
pub mod image;
pub mod layer1;
pub mod reference;
pub mod standin;

use std::path::Path;

/// Runs a program from a host directory on our kernel and in the reference, with the same arguments, in
/// a container of the reference image with the directory at `/wc`, and returns both.
pub fn run_both(program: &Path, arguments: &[&str]) -> (driver::Run, driver::Outcome) {
    let name = format!(
        "/wc/{}",
        program
            .file_name()
            .expect("program has a name")
            .to_string_lossy()
    );
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
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(directory, "wc"));
    let container = driver::Container {
        files: &files,
        path,
        arguments,
        environment: reference::ENVIRONMENT,
        working_directory: "/",
    };
    driver::run(&container, 0)
        .unwrap_or_else(|failure| panic!("no outcome on our kernel: {failure:?}"))
}
