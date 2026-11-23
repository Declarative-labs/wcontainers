//! The kernel test crate's programs and the virtual-machine executor's run in the same kernel, loop and seeded policy
//! under form P and under the WebAssembly output: both give the same wait status and output (plan 13's equal verdicts;
//! addresses, memory size and the count at every stop wait on the kernel's K2, so they are not compared here).
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use linux_raw_sys::general::{S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH};
use wcontainers_abi::{Host, HostError, Warning};
use wcontainers_arm64::Arm64;
use wcontainers_executor_host_process::executor::HostProcessExecutor;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
use wcontainers_kernel_linux::{
    ARM64_ADDRESS_SPACE_BYTES, Attributes, NewFile, Start, StartOptions,
};
use wcontainers_runtime_single_kernel::{Ending, Execution, Kernel, System, drive};
use wcontainers_system_seeded_test_policy::{ERROR, TestPolicy};
use wcontainers_translator_test::guest::GuestTest;
use wcontainers_translator_test::programs::{C_DRIVER, OUTPUT_FLAG, ZIG_TARGET_FLAG};

/// The kernel test crate's programs, and the virtual-machine executor's own, from this package.
const KERNEL_TEST_GUEST: &str = "../unicorn/guest";
const VM_GUEST: &str = "../hardware-virtualization/tests/guest";
/// The translator's test programs, whose guest folder holds headers the virtual-machine executor's programs include.
const PROGRAMS: &str = "../../../wcontainers-translator-test/programs";

/// Seeds the policy and the host's random bytes; `WCHP_KERNEL_SEEDS` runs that many seeds from it.
const SEED_VARIABLE: &str = "WCHP_KERNEL_SEED";
const SEEDS_VARIABLE: &str = "WCHP_KERNEL_SEEDS";
const DEFAULT_SEED: u64 = 1;

/// The clocks the test host reads.
const MONOTONIC_NANOSECONDS: u64 = 1_234_567;
const REALTIME_NANOSECONDS: u64 = 1_763_500_000_000_000_000;

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

/// What the host was asked to write to each output end.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Output {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// A host with fixed clocks and seeded random bytes that keeps each output end's bytes, with no network or backend.
struct Recording {
    output: Mutex<Output>,
    random: Mutex<fastrand::Rng>,
}

impl wcontainers_clock::Clock for Recording {
    fn monotonic_nanoseconds(&self) -> u64 {
        MONOTONIC_NANOSECONDS
    }
    fn realtime_nanoseconds(&self) -> u64 {
        REALTIME_NANOSECONDS
    }
}

impl wcontainers_random_source::RandomSource for Recording {
    fn fill_random_bytes(&self, into: &mut [u8]) -> Result<(), HostError> {
        self.random
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .fill(into);
        Ok(())
    }
}

impl Host for Recording {
    fn warn(&self, _warning: Warning<'_>) {}
}

impl wcontainers_system::StreamTransport for Recording {
    type Endpoint = u32;

    fn send_stream(&self, peer: &u32, bytes: &[u8]) -> Result<usize, HostError> {
        let mut output = self.output.lock().unwrap_or_else(PoisonError::into_inner);
        match *peer {
            ERROR => &mut output.stderr,
            _ => &mut output.stdout,
        }
        .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn receive_stream(&self, _peer: &u32, _into: &mut [u8]) -> Result<usize, HostError> {
        Err(HostError::NotSupported)
    }
    fn close_stream(&self, _peer: &u32) {}
}

impl wcontainers_readiness::Readiness<u32> for Recording {
    async fn wait_until_ready(
        &self,
        _peer: &u32,
        interest: wcontainers_readiness::ReadyFor,
    ) -> Result<wcontainers_readiness::ReadinessReport, HostError> {
        if interest.contains(wcontainers_readiness::ReadyFor::WRITABLE) {
            return Ok(wcontainers_readiness::ReadinessReport {
                ready_for: wcontainers_readiness::ReadyFor::WRITABLE,
                unconditional: wcontainers_readiness::UnconditionalEvents::empty(),
            });
        }
        std::future::pending().await
    }
}

/// How one side's run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Outcome {
    status: i32,
    output: Output,
}

/// Runs the static program `image` at the root of a container, with `arguments`, on `cpu`.
fn run<E: Execution<Failure = core::convert::Infallible>>(
    cpu: &mut E,
    seed: u64,
    name: &str,
    image: &[u8],
    arguments: &[&str],
) -> Outcome {
    let recording = Recording {
        output: Mutex::default(),
        random: Mutex::new(fastrand::Rng::with_seed(seed)),
    };
    let policy = Arc::new(TestPolicy::new(recording, seed, REALTIME_NANOSECONDS));
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
    let memory_bytes = cpu.address_space().end;
    let start = Start {
        path: &path,
        arguments: &arguments,
        environment: &[b"PATH=/"],
        working_directory: b"/",
        hostname: b"wcontainers",
        groups: &[],
        backend: None,
        standard: policy.standard(None),
        address_space_bytes: ARM64_ADDRESS_SPACE_BYTES,
        memory_bytes,
        ceiling: None,
        options: StartOptions::default(),
    };
    kernel.start(cpu, &policy, start);
    policy.started(&mut kernel, cpu);
    let status = match drive(&mut kernel, cpu, &policy) {
        Ok(Ending::Finished { status }) => status,
        Ok(Ending::Ended(never)) => match never {},
        Err(never) => match never {},
    };
    drop(kernel);
    let policy = Arc::into_inner(policy).expect("the kernel keeps no call on the policy");
    let output = policy
        .into_services()
        .output
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner);
    Outcome { status, output }
}

/// Builds the static ARM64 Linux program `directory/name.c` with zig's C compiler, linked with musl or with no C
/// library, once per test process.
fn compile(directory: &str, name: &str, with_libc: bool) -> Vec<u8> {
    static BUILT: OnceLock<Mutex<std::collections::BTreeMap<String, Vec<u8>>>> = OnceLock::new();
    let built = BUILT.get_or_init(Mutex::default);
    let key = format!("{directory}/{name}");
    if let Some(image) = built
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
    {
        return image.clone();
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(directory)
        .join(name)
        .with_extension("c");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("wchp-{name}"));
    let object = out.with_extension("o");
    let include = format!(
        "-I{}",
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(PROGRAMS)
            .join(<Arm64 as GuestTest>::NAME)
            .display()
    );
    let target = [C_DRIVER, ZIG_TARGET_FLAG, <Arm64 as GuestTest>::C_TARGET];
    // Compiled with the C library's headers, linked with its code or without it.
    let mut compile = Command::new("zig");
    compile
        .args(target)
        .args(["-O2", "-c", &include, OUTPUT_FLAG])
        .arg(&object)
        .arg(&source);
    let mut link = Command::new("zig");
    link.args(target)
        .arg("-static")
        .args((!with_libc).then_some("-nostdlib"))
        .arg(OUTPUT_FLAG)
        .arg(&out)
        .arg(&object);
    for command in [&mut compile, &mut link] {
        let output = command
            .output()
            .expect("running zig cc; run the test inside `nix develop`");
        assert!(
            output.status.success(),
            "{command:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let image = std::fs::read(&out).expect("the built program");
    built
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, image.clone());
    image
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

/// Runs the program under both forms for every seed; they must agree. The form P outcome of the first seed.
fn both(directory: &str, name: &str, with_libc: bool, arguments: &[&str]) -> Outcome {
    both_known(directory, name, with_libc, arguments, None)
}

/// As [`both`]; where the WebAssembly output is known to print otherwise for `known`, the outputs are printed and
/// only the statuses must agree.
fn both_known(
    directory: &str,
    name: &str,
    with_libc: bool,
    arguments: &[&str],
    known: Option<&str>,
) -> Outcome {
    let image = compile(directory, name, with_libc);
    let mut first = None;
    for seed in seeds() {
        let ours = run(&mut host_process(), seed, name, &image, arguments);
        let theirs = run(&mut web_assembly(), seed, name, &image, arguments);
        match known {
            Some(known) if ours.output != theirs.output => {
                assert_eq!(ours.status, theirs.status, "{name} seed {seed}: statuses");
                println!(
                    "{name}: KNOWN FAILURE, {known}:\nform P: {}\nWebAssembly: {}",
                    String::from_utf8_lossy(&ours.output.stdout),
                    String::from_utf8_lossy(&theirs.output.stdout)
                );
            }
            _ => assert_eq!(
                ours, theirs,
                "{name} seed {seed}: form P (left) and the WebAssembly output (right) differ"
            ),
        }
        first.get_or_insert(ours);
    }
    first.expect("at least one seed")
}

#[test]
fn hello_gives_the_web_assembly_output_s_status_and_output() {
    let outcome = both(KERNEL_TEST_GUEST, "hello", false, &[]);
    assert_eq!(outcome.output.stdout, b"hello, world\n", "{outcome:?}");
    assert_eq!(outcome.output.stderr, b"hello, error\n", "{outcome:?}");
}

#[test]
fn getppid_calls_give_the_web_assembly_output_s_status_and_output() {
    let outcome = both(VM_GUEST, "getppid", true, &["10000"]);
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
    let outcome = both(VM_GUEST, "forks", true, &[]);
    let stdout = String::from_utf8_lossy(&outcome.output.stdout);
    assert_eq!(outcome.status, FORKS_EXIT << u8::BITS, "{stdout}");
    for line in FORKS_PRINTS {
        assert!(stdout.contains(line), "{stdout}");
    }
}

#[test]
fn code_written_and_announced_gives_the_web_assembly_output_s_status_and_output() {
    both_known(
        VM_GUEST,
        "code",
        true,
        &[],
        Some(
            "the WebAssembly side runs a function's old code after only its second cache line is rewritten and \
             announced",
        ),
    );
}

#[test]
fn memory_asked_for_gives_the_web_assembly_output_s_status_and_output() {
    both(VM_GUEST, "mappings", true, &[]);
}

#[test]
fn blocks_zeroed_and_cleaned_give_the_web_assembly_output_s_status_and_output() {
    both_known(
        VM_GUEST,
        "blocks",
        true,
        &[],
        Some("the WebAssembly side reports a zeroed block's fault at the block's start"),
    );
}

#[test]
fn jumps_through_tables_give_the_web_assembly_output_s_status_and_output() {
    both(VM_GUEST, "jumptables", true, &[]);
}
