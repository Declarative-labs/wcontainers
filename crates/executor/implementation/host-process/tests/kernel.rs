//! The kernel test crate's programs and the virtual-machine executor's run in the same kernel, loop and seeded policy
//! under form P and under the WebAssembly output, both containers given the WebAssembly output's ceiling as their
//! ceiling and memory size: both give the same wait status, output and root files, and a program with one thread whose
//! turns end only at its system calls the same turns. `layout` prints its addresses, memory size and mappings. Where
//! the WebAssembly output is known to print otherwise, form P's status and output are the program's run on this host.
//! Turns limited to one counted event change nothing under form P, and under its exact executor a stop at every counted
//! event gives the virtual-machine executor's count and pc at every stop.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

mod guest_build;
mod kernel_host;

use std::sync::{Arc, OnceLock};

use guest_build::{PROGRAMS, compile, freestanding};
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
/// container's ceiling and memory size (none: no ceiling, and the executor's address space as its memory size), every
/// turn ending after `turn_events` counted events where that is given.
fn run<E: Execution<Failure = core::convert::Infallible>>(
    cpu: &mut E,
    limit: Option<u64>,
    turn_events: Option<u64>,
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
        limit,
        turn_events,
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

fn host() -> Arc<HostProcessHost> {
    HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
        .clone()
}

fn host_process() -> HostProcessExecutor {
    HostProcessExecutor::new(host())
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
    /// In wait status alone, where the WebAssembly output is known to print otherwise for the reason given; form P's
    /// status and output are then the program's run on this host.
    Known(&'a str),
}

/// The program `image` run on this host as the reference, as the container runs it: its wait status and output.
fn on_this_host(name: &str, image: &[u8], arguments: &[&str]) -> (i32, Output) {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("kernel-reference");
    std::fs::create_dir_all(&directory).expect("the reference's folder");
    let path = directory.join(name);
    std::fs::write(&path, image).expect("the reference's program");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(S_IRWXU))
        .expect("the reference's program runs");
    let ran = std::process::Command::new(&path)
        .arg0(format!("/{name}"))
        .args(arguments)
        .env_clear()
        .env("PATH", "/")
        .output()
        .expect("the reference runs");
    let output = Output {
        stdout: ran.stdout,
        stderr: ran.stderr,
    };
    (ran.status.into_raw(), output)
}

/// Runs the program under both forms for every seed; they must agree as `agree` says. Form P's outcome for each seed.
fn both(
    directory: &str,
    name: &str,
    with_libc: bool,
    arguments: &[&str],
    agree: Agree<'_>,
) -> Vec<(u64, Outcome)> {
    let image = compile(directory, name, with_libc);
    let limit = web_assembly().address_space().end;
    let mut outcomes = Vec::new();
    for seed in seeds() {
        let ours = run(
            &mut host_process(),
            Some(limit),
            None,
            seed,
            name,
            &image,
            arguments,
        );
        let theirs = run(
            &mut web_assembly(),
            Some(limit),
            None,
            seed,
            name,
            &image,
            arguments,
        );
        let whole = |outcome: &Outcome| {
            (
                outcome.status,
                outcome.output.clone(),
                outcome.files.clone(),
            )
        };
        if let Agree::Known(_) = agree {
            let (status, output) = on_this_host(name, &image, arguments);
            assert_eq!(
                (ours.status, &ours.output),
                (status, &output),
                "{name} seed {seed}: form P (left) and the program on this host (right) differ"
            );
        }
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
        outcomes.push((seed, ours));
    }
    outcomes
}

/// Form P's outcome for the first seed.
fn first(outcomes: Vec<(u64, Outcome)>) -> Outcome {
    outcomes.into_iter().next().expect("at least one seed").1
}

#[test]
fn hello_gives_the_web_assembly_output_s_status_and_output() {
    let outcome = first(both(KERNEL_TEST_GUEST, "hello", false, &[], Agree::Alone));
    assert_eq!(outcome.output.stdout, b"hello, world\n", "{outcome:?}");
    assert_eq!(outcome.output.stderr, b"hello, error\n", "{outcome:?}");
}

#[test]
fn getppid_calls_give_the_web_assembly_output_s_status_and_output() {
    let outcome = first(both(VM_GUEST, "getppid", true, &["10000"], Agree::Alone));
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
    let outcome = first(both(VM_GUEST, "forks", true, &[], Agree::Whole));
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
    let outcome = first(both(OWN_GUEST, "layout", true, &[], Agree::Alone));
    let stdout = String::from_utf8_lossy(&outcome.output.stdout);
    assert_eq!(outcome.status, 0, "{stdout}");
    assert!(stdout.contains("[stack]"), "{stdout}");
    print!("{stdout}");
}

/// Runs the program under form P with every turn limited to one counted event, for each seed its run without
/// (`without`): the same wait status, output and root files, in more turns. Form P ends such a turn at the first exit
/// at or after the limit or when the slice's stop lands, not at the counted event itself.
fn one_event_turns(directory: &str, name: &str, arguments: &[&str], without: &[(u64, Outcome)]) {
    let image = compile(directory, name, true);
    let limit = web_assembly().address_space().end;
    for (seed, without) in without {
        let limited = run(
            &mut host_process(),
            Some(limit),
            Some(1),
            *seed,
            name,
            &image,
            arguments,
        );
        assert_eq!(
            (limited.status, &limited.output, &limited.files),
            (without.status, &without.output, &without.files),
            "{name} seed {seed}: turns limited to one counted event changed the run"
        );
        assert!(
            limited.turns > without.turns,
            "{name} seed {seed}: {} turns limited to one counted event, {} without",
            limited.turns,
            without.turns
        );
        println!(
            "{name} seed {seed}: turns limited to one counted event, {} of them, changed nothing",
            limited.turns
        );
    }
}

#[test]
fn turns_limited_to_one_counted_event_change_nothing() {
    let jump_tables = both(VM_GUEST, "jumptables", true, &[], Agree::Whole);
    one_event_turns(VM_GUEST, "jumptables", &[], &jump_tables);
    let layout = both(OWN_GUEST, "layout", true, &[], Agree::Alone);
    one_event_turns(OWN_GUEST, "layout", &[], &layout);
    let forks = both(VM_GUEST, "forks", true, &[], Agree::Whole);
    one_event_turns(VM_GUEST, "forks", &[], &forks);
}

/// The program `name` with a stop at every counted event under form P's exact executor, for every seed: the run's
/// status and output, no turn past its limit, and the virtual-machine executor's returns and hash of their positions
/// (count and pc), `returns` and `hash`, which it gives alike at look-ahead none and 256 (docs/status-native.md, N4 runs
/// 15 to 21, and on Graviton4 again in round 7, scratch w1/r7run1/vm1-arm.out). Benchmarks are built as the
/// virtual-machine executor's kernel test builds them.
fn every_event_against_the_virtual_machine(name: &str, returns: u64, hash: u64) {
    use wcontainers_executor_host_process::executor::native::reason::TURN_LIMIT_PASSED;
    let image = if name == "jumptables" {
        compile(VM_GUEST, name, true)
    } else {
        std::fs::read(freestanding(PROGRAMS, name, name, &[], &[RESERVE_X18]))
            .expect("the built program")
    };
    for seed in seeds() {
        let without = run(&mut host_process(), None, None, seed, name, &image, &[]);
        let mut exact = HostProcessExecutor::exact(host());
        let every = run(&mut exact, None, Some(1), seed, name, &image, &[]);
        assert_eq!(
            (every.status, &every.output),
            (without.status, &without.output),
            "{name} seed {seed}: a stop at every counted event changed the run"
        );
        assert!(
            !exact
                .slow_paths()
                .iter()
                .any(|(reason, _)| *reason == TURN_LIMIT_PASSED),
            "{name} seed {seed}: a turn ended past its limit"
        );
        println!(
            "{name} seed {seed}: a stop at every counted event, (returns, hash of positions) {:?}, slow paths {:?}",
            exact.positions(),
            exact.slow_paths()
        );
        assert_eq!(
            exact.positions(),
            (returns, hash),
            "{name} seed {seed}: (returns, hash of positions) against the virtual-machine executor's"
        );
    }
}

/// The compiler flag the virtual-machine executor's kernel test builds the benchmark set with.
const RESERVE_X18: &str = "-mcpu=generic+reserve_x18";

#[test]
fn count_at_every_stop_of_bench_is_the_virtual_machine_executor_s() {
    every_event_against_the_virtual_machine("bench", 177_072, 10_937_141_521_840_235_265);
}

#[test]
fn count_at_every_stop_of_calls_is_the_virtual_machine_executor_s() {
    every_event_against_the_virtual_machine("calls", 3_004, 18_260_131_204_484_228_776);
}

/// Its jumps to misaligned targets end a limited turn at the target before the target's fault.
#[test]
fn count_at_every_stop_of_jumptables_is_the_virtual_machine_executor_s() {
    every_event_against_the_virtual_machine("jumptables", 5_484, 17_339_527_552_300_285_948);
}
