//! The kernel test crate's programs and the virtual-machine executor's run in the same kernel, loop and seeded policy
//! under form P and under the WebAssembly output, both containers given the WebAssembly output's ceiling as their
//! ceiling and memory size: both give the same wait status, output and root files, and a program with one thread whose
//! turns end only at its system calls the same turns. `layout` prints its addresses, memory size and mappings.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

#[expect(dead_code, reason = "the freestanding build is the speed test's")]
mod guest_build;
mod kernel_host;

use std::sync::{Arc, OnceLock};

use guest_build::compile;
use kernel_host::{Output, Program, REALTIME_NANOSECONDS, run_container};
use linux_raw_sys::general::{S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH};
use wcontainers_abi::Cpu;
use wcontainers_arm64::Arm64;
use wcontainers_executor_host_process::executor::HostProcessExecutor;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
use wcontainers_kernel_linux::{Attributes, FileRecord, NewFile};
use wcontainers_runtime_single_kernel::{Execution, Kernel};

/// The kernel test crate's programs, and the virtual-machine executor's own, from this package.
const KERNEL_TEST_GUEST: &str = "../unicorn/guest";
const VM_GUEST: &str = "../hardware-virtualization/tests/guest";
/// This package's own programs.
const OWN_GUEST: &str = "tests/guest";

/// Seeds the policy and the host's random bytes; `WCHP_KERNEL_SEEDS` runs that many seeds from it.
const SEED_VARIABLE: &str = "WCHP_KERNEL_SEED";
const SEEDS_VARIABLE: &str = "WCHP_KERNEL_SEEDS";
const DEFAULT_SEED: u64 = 1;

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

/// How one side's run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    output: Output,
    files: Vec<FileRecord>,
    turns: u64,
}

/// Runs the static program `image` at the root of a container, with `arguments`, on `cpu`, with `limit` as the
/// container's ceiling and memory size.
fn run<E: Execution<Failure = core::convert::Infallible>>(
    cpu: &mut E,
    limit: u64,
    seed: u64,
    name: &str,
    image: &[u8],
    arguments: &[&str],
) -> Outcome {
    let mut kernel = Kernel::new();
    let path = format!("/{name}").into_bytes();
    kernel
        .add_file(
            &path,
            Attributes {
                mode: S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH,
                uid: 0,
                gid: 0,
                mtime_seconds: 0,
            },
            NewFile::Regular(image),
        )
        .expect("the program is taken");
    let arguments: Vec<&[u8]> = std::iter::once(path.as_slice())
        .chain(arguments.iter().map(|a| a.as_bytes()))
        .collect();
    let program = Program {
        path: &path,
        arguments: &arguments,
        environment: &[b"PATH=/"],
        hostname: b"wcontainers",
        groups: &[],
        limit: Some(limit),
    };
    let ran = run_container(cpu, kernel, seed, REALTIME_NANOSECONDS, &program);
    assert!(
        ran.files.iter().any(|file| file.path == path),
        "{name}: the program's file is gone from the root"
    );
    Outcome {
        status: ran.status,
        output: ran.output,
        files: ran.files,
        turns: ran.turns,
    }
}

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

fn host_process() -> HostProcessExecutor {
    HostProcessExecutor::new(
        HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
            .clone(),
    )
}

fn web_assembly() -> CpuSide<Arm64> {
    CpuSide::new(
        Compiler::<Arm64>::new(None),
        LookAhead::None,
        Optimise::Speed,
        (),
    )
}

/// How the two sides' outcomes must agree.
#[derive(Clone, Copy)]
enum Agree<'a> {
    /// In wait status, output and root files.
    Whole,
    /// As whole, and in turns: a program with one thread whose turns end only at its system calls.
    Alone,
    /// In wait status alone, where the WebAssembly output is known to print otherwise for the reason given.
    Known(&'a str),
}

/// Runs the program under both forms for every seed; they must agree as `agree` says. The form P outcome of the first
/// seed.
fn both(
    directory: &str,
    name: &str,
    with_libc: bool,
    arguments: &[&str],
    agree: Agree<'_>,
) -> Outcome {
    let image = compile(directory, name, with_libc);
    let limit = web_assembly().address_space().end;
    let mut first = None;
    for seed in seeds() {
        let ours = run(&mut host_process(), limit, seed, name, &image, arguments);
        let theirs = run(&mut web_assembly(), limit, seed, name, &image, arguments);
        let whole = |outcome: &Outcome| {
            (
                outcome.status,
                outcome.output.clone(),
                outcome.files.clone(),
            )
        };
        match agree {
            Agree::Known(known) if ours.output != theirs.output => {
                assert_eq!(ours.status, theirs.status, "{name} seed {seed}: statuses");
                println!(
                    "{name}: KNOWN FAILURE, {known}:\nform P: {}\nWebAssembly: {}",
                    String::from_utf8_lossy(&ours.output.stdout),
                    String::from_utf8_lossy(&theirs.output.stdout)
                );
            }
            Agree::Alone => assert_eq!(
                ours, theirs,
                "{name} seed {seed}: form P (left) and the WebAssembly output (right) differ"
            ),
            _ => assert_eq!(
                whole(&ours),
                whole(&theirs),
                "{name} seed {seed}: form P (left) and the WebAssembly output (right) differ"
            ),
        }
        println!(
            "{name} seed {seed}: {} turns under form P, {} under the WebAssembly output",
            ours.turns, theirs.turns
        );
        first.get_or_insert(ours);
    }
    first.expect("at least one seed")
}

#[test]
fn hello_gives_the_web_assembly_output_s_status_and_output() {
    let outcome = both(KERNEL_TEST_GUEST, "hello", false, &[], Agree::Alone);
    assert_eq!(outcome.output.stdout, b"hello, world\n", "{outcome:?}");
    assert_eq!(outcome.output.stderr, b"hello, error\n", "{outcome:?}");
}

#[test]
fn getppid_calls_give_the_web_assembly_output_s_status_and_output() {
    let outcome = both(VM_GUEST, "getppid", true, &["10000"], Agree::Alone);
    assert_eq!(outcome.status, 0, "{outcome:?}");
}

/// The forks program's exit code, and what its parent prints after its children.
const FORKS_EXIT: i32 = 3;
const FORKS_PRINTS: [&str; 2] = [
    "parent: signal 10 taken, child exited 7\n",
    "parent: second child killed by 15\n",
];

#[test]
fn forks_execs_and_signals_give_the_web_assembly_output_s_status_and_output() {
    let outcome = both(VM_GUEST, "forks", true, &[], Agree::Whole);
    let stdout = String::from_utf8_lossy(&outcome.output.stdout);
    assert_eq!(outcome.status, FORKS_EXIT << u8::BITS, "{stdout}");
    for line in FORKS_PRINTS {
        assert!(stdout.contains(line), "{stdout}");
    }
}

#[test]
fn code_written_and_announced_gives_the_web_assembly_output_s_status_and_output() {
    both(
        VM_GUEST,
        "code",
        true,
        &[],
        Agree::Known(
            "the WebAssembly side runs a function's old code after only its second cache line is rewritten and \
             announced",
        ),
    );
}

#[test]
fn memory_asked_for_gives_the_web_assembly_output_s_status_and_output() {
    both(VM_GUEST, "mappings", true, &[], Agree::Whole);
}

#[test]
fn blocks_zeroed_and_cleaned_give_the_web_assembly_output_s_status_and_output() {
    both(
        VM_GUEST,
        "blocks",
        true,
        &[],
        Agree::Known("the WebAssembly side reports a zeroed block's fault at the block's start"),
    );
}

#[test]
fn jumps_through_tables_give_the_web_assembly_output_s_status_and_output() {
    both(VM_GUEST, "jumptables", true, &[], Agree::Whole);
}

#[test]
fn addresses_memory_size_and_mappings_are_the_web_assembly_output_s() {
    let outcome = both(OWN_GUEST, "layout", true, &[], Agree::Alone);
    let stdout = String::from_utf8_lossy(&outcome.output.stdout);
    assert_eq!(outcome.status, 0, "{stdout}");
    assert!(stdout.contains("[stack]"), "{stdout}");
    print!("{stdout}");
}
