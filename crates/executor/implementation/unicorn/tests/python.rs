//! CPython from an unmodified Alpine image (layer 2): modules of its own test suite and a program
//! using child processes, threads, sockets on loopback and the file system, each compared with
//! the reference under several schedules with saves.

use std::path::{Path, PathBuf};

use wcontainers_executor_unicorn::reference::{PLATFORM, docker};
use wcontainers_executor_unicorn::runtimes::{self, PYTHON};

/// This crate's Python programs, with CPython's `Lib/test` from the source release the image was
/// built from (its checksum is the image's `PYTHON_SHA256`) at `lib/test`, fetched once.
fn directory() -> PathBuf {
    static DIRECTORY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIRECTORY.get_or_init(prepare).clone()
}

fn prepare() -> PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("python");
    let lib = out.join("lib");
    if !lib.join("test").join("__init__.py").exists() {
        std::fs::create_dir_all(&lib).expect("a directory for the tests");
        let fetched = std::process::Command::new(docker::CLIENT)
            .args([
            docker::START_CONTAINER,
            docker::REMOVE_ON_EXIT,
            docker::PLATFORM_OPTION,
            PLATFORM,
            docker::VOLUME_OPTION,
        ])
            .arg(format!("{}:/out", lib.canonicalize().expect("the directory").display()))
            .args([PYTHON, "sh", "-c"])
            .arg(
                "set -e; cd /tmp; \
                 wget -q https://www.python.org/ftp/python/$PYTHON_VERSION/Python-$PYTHON_VERSION.tar.xz; \
                 echo \"$PYTHON_SHA256  Python-$PYTHON_VERSION.tar.xz\" | sha256sum -c -; \
                 tar -xJf Python-$PYTHON_VERSION.tar.xz Python-$PYTHON_VERSION/Lib/test; \
                 rm -rf /out/test.partial; mv Python-$PYTHON_VERSION/Lib/test /out/test.partial; \
                 mv /out/test.partial /out/test",
            )
            .status()
            .expect("running docker");
        assert!(fetched.success(), "fetching CPython's tests");
    }
    for entry in std::fs::read_dir(runtimes::programs("python")).expect("the programs") {
        let entry = entry.expect("an entry");
        std::fs::copy(entry.path(), out.join(entry.file_name())).expect("copying a program");
    }
    out
}

/// Two saves, at most `gap` turns apart: about a third of the run's turns (measured).
fn saves(gap: u64) -> runtimes::Saves {
    runtimes::Saves { count: 2, gap }
}

#[test]
fn system_program_matches_reference() {
    let directory = directory();
    let runs = runtimes::assert_same(
        PYTHON,
        &directory,
        &["python3", "/wc/system.py"],
        saves(4_000),
    );
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}

/// The test suite's modules for the language's own types and library.
const LANGUAGE_MODULES: &[&str] = &[
    "test_bool",
    "test_list",
    "test_dict",
    "test_tuple",
    "test_struct",
    "test_functools",
    "test_collections",
    "test_string",
    "test_generators",
    "test_with",
    "test_contextlib",
    "test_re",
];

/// The test suite's modules for the operating system.
const SYSTEM_MODULES: &[&str] = &[
    "test_posix",
    "test_fileio",
    "test_file",
    "test_tempfile",
    "test_shutil",
    "test_glob",
    "test_pty",
    "test_mmap",
    "test_epoll",
    "test_fcntl",
    "test_selectors",
];

#[test]
fn language_test_modules_match_reference() {
    let directory = directory();
    let mut command = vec!["python3", "/wc/suite.py"];
    command.extend_from_slice(LANGUAGE_MODULES);
    let runs = runtimes::assert_same(PYTHON, &directory, &command, saves(13_000));
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}

#[test]
fn system_test_modules_match_reference() {
    let directory = directory();
    let mut command = vec!["python3", "/wc/suite.py"];
    command.extend_from_slice(SYSTEM_MODULES);
    let runs = runtimes::assert_same(PYTHON, &directory, &command, saves(60_000));
    eprintln!("warned: {:?}", runtimes::warned(&runs));
}
