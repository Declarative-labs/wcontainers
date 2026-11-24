//! Form P's speed figures (plan 13's W1 bar), each a measurement run on its own with `--ignored --nocapture`:
//! - the guest's getppid round trip, translated code calling the system call's floor entry and answered by the
//!   test's loop at once, timed at every exit (knobs `WCHP_CALLS`, `WCHP_PER_BATCH`);
//! - bench.c's eight workloads run plainly as host programs, under form P and under the WebAssembly output, each in
//!   the kernel's loop with the seeded test policy, in alternating rounds (knob `WCHP_ROUNDS`), outputs checked equal;
//! - busybox `sh` starting programs in a loop (knob `WCHP_PROGRAMS`), twice on one executor, the second reusing the
//!   first's translations, against the reference's own time in Docker and the model of plan 13.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

mod guest_build;
mod kernel_host;

use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use guest_build::{PROGRAMS, compile, freestanding};
use kernel_host::{Program, REALTIME_NANOSECONDS, run_container};
use linux_raw_sys::general::{S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH};
use wcontainers_abi::{Cpu, Exit, Permissions, RegisterBlock, Resume, Warning};
use wcontainers_arm64::Arm64;
use wcontainers_arm64::encode::{self, form};
use wcontainers_executor_host_process::executor::HostProcessExecutor;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_unicorn::busybox::on_path;
use wcontainers_executor_unicorn::{image, reference};
use wcontainers_executor_wasmtime::{Compiler, CpuSide, LookAhead, Optimise};
use wcontainers_kernel_linux::{Attributes, NewFile};
use wcontainers_runtime_single_kernel::{Answer, Carry, Execution, Kernel, Leaving, Turn};

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

fn host() -> Arc<HostProcessHost> {
    HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused")))
        .clone()
}

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn load() -> String {
    std::fs::read_to_string("/proc/loadavg").unwrap_or_default()
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

const PAGE: u64 = 4096;
const CODE: u64 = 0x40_0000;
const INSTRUCTION: i64 = size_of::<u32>() as i64;
const GETPPID: u16 = linux_raw_sys::general::__NR_getppid as u16;
const EXIT: u16 = linux_raw_sys::general::__NR_exit as u16;
const SYSCALL_NUMBER: u8 = 8;
/// The register the loop counts down in.
const COUNTER: u8 = 19;
/// What the test answers getppid with.
const PARENT: u64 = 1;

/// Times every exit of a thread: each getppid is answered at once with [`PARENT`]; exit leaves.
struct Timed {
    last: Instant,
    /// Nanoseconds from each exit to the next.
    samples: Vec<u32>,
}

impl Answer for Timed {
    fn exit(&mut self, _cpu: &mut dyn Cpu, exit: Exit, _progress: u64) -> Carry {
        let now = Instant::now();
        let gap = now.duration_since(self.last).as_nanos();
        self.last = now;
        match exit {
            Exit::Syscall(call) if call.number.0 == u64::from(GETPPID) => {
                self.samples.push(u32::try_from(gap).unwrap_or(u32::MAX));
                Carry::On {
                    resume: Resume::Return(PARENT),
                    turn: Turn::Free,
                }
            }
            Exit::Syscall(call) if call.number.0 == u64::from(EXIT) => {
                Carry::Leave(Leaving::Finished)
            }
            other => panic!("the loop came back with {other:?}"),
        }
    }

    fn clock(&mut self, _progress: u64) -> u64 {
        0
    }

    fn warn(&mut self, warning: Warning<'_>) {
        println!("warned: {} x{}", warning.reason, warning.count);
    }
}

/// `calls` getppid calls in a loop on a new memory of `cpu`, every exit timed.
fn getppid_loop(cpu: &mut HostProcessExecutor, calls: u64) -> Vec<u32> {
    let words = [
        encode::movz(SYSCALL_NUMBER, GETPPID, 0),
        encode::exception(form::SVC, 0),
        encode::add_immediate(COUNTER, COUNTER, -1).expect("an add"),
        encode::compare_branch(true, COUNTER, -3 * INSTRUCTION).expect("cbnz"),
        encode::movz(SYSCALL_NUMBER, EXIT, 0),
        encode::exception(form::SVC, 0),
    ];
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let memory = cpu.memory_new().expect("memory");
    cpu.set_permissions(memory, CODE, PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code");
    cpu.write(memory, CODE, &bytes).expect("the code");
    cpu.code_changed(memory, CODE, PAGE, &[CODE]);
    let thread = cpu.thread_new(memory).expect("thread");
    let mut registers = RegisterBlock::ZERO;
    registers.pc = CODE;
    registers.x[usize::from(COUNTER)] = calls;
    cpu.set_registers(thread, &registers);
    let mut timed = Timed {
        last: Instant::now(),
        samples: Vec::with_capacity(calls as usize),
    };
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, Turn::Free, &mut timed);
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    assert_eq!(timed.samples.len() as u64, calls, "every call came back");
    timed.samples
}

/// The guest's getppid round trip: the median over batches of the mean per call (as W0's futex slot reports it),
/// and the per-call median, tail and largest, with the count over 1 ms.
#[test]
#[ignore = "a measurement, run on its own"]
fn getppid_round_trip() {
    let calls = knob("WCHP_CALLS", 1_000_000);
    let per_batch = knob("WCHP_PER_BATCH", 10_000) as usize;
    let mut cpu = HostProcessExecutor::new(host());
    getppid_loop(&mut cpu, 10_000);
    let started = Instant::now();
    let samples = getppid_loop(&mut cpu, calls);
    let elapsed = started.elapsed();
    // The first call's gap holds the turn's start.
    let samples = &samples[1..];
    let mut batches: Vec<f64> = samples
        .chunks_exact(per_batch)
        .map(|batch| batch.iter().map(|&n| f64::from(n)).sum::<f64>() / per_batch as f64)
        .collect();
    let batch_median = median(&mut batches);
    let mut sorted: Vec<u32> = samples.to_vec();
    sorted.sort_unstable();
    let at = |share: f64| sorted[((sorted.len() - 1) as f64 * share) as usize];
    let over = sorted.iter().filter(|&&n| n > 1_000_000).count();
    println!(
        "getppid round trip on cpu {:?}: median of batch means {batch_median:.0} ns ({} batches of {per_batch}), per call median {} ns, 99% {} ns, 99.99% {} ns, largest {} ns, {over} over 1 ms of {} calls in {elapsed:?}, kernel entries {}, load {}",
        cpu.runner_cpu(),
        batches.len(),
        at(0.5),
        at(0.99),
        at(0.9999),
        sorted[sorted.len() - 1],
        samples.len(),
        cpu.counters().kernel_entries,
        load().trim()
    );
    println!("slow paths: {:?}", cpu.slow_paths());
}

/// bench.c's workloads, in the order of its `WORKLOAD` numbers, which start at one.
const WORKLOADS: [&str; 8] = [
    "fib",
    "sieve",
    "hash",
    "sort",
    "interpret",
    "matrix",
    "walk",
    "search",
];

/// A static program run at the root of a container in the kernel's loop on `cpu`: its wait status and output.
fn in_kernel<E: Execution<Failure = core::convert::Infallible>>(
    cpu: &mut E,
    name: &str,
    image: &[u8],
    arguments: &[&str],
) -> (i32, Vec<u8>) {
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
    let mut all: Vec<&[u8]> = vec![&path];
    all.extend(arguments.iter().map(|argument| argument.as_bytes()));
    let program = Program {
        path: &path,
        arguments: &all,
        environment: &[b"PATH=/"],
        hostname: b"wcontainers",
        groups: &[],
    };
    let ran = run_container(cpu, kernel, 1, REALTIME_NANOSECONDS, &program);
    assert!(
        ran.files.iter().any(|file| file.path == path),
        "{name}: the program's file is gone from the root"
    );
    (ran.status, ran.output.stdout)
}

fn web_assembly() -> CpuSide<Arm64> {
    CpuSide::new(
        Compiler::<Arm64>::new(None),
        LookAhead::Full,
        Optimise::Speed,
        (),
    )
}

/// Each workload's wall time plainly, under form P and under the WebAssembly output, every run whole (start and
/// translation included), in alternating rounds; the median of each and the ratios to the plain run.
#[test]
#[ignore = "a measurement, run on its own"]
fn bench_workloads_plainly_under_p_and_under_web_assembly() {
    let rounds = knob("WCHP_ROUNDS", 5);
    println!("load at the start {}", load().trim());
    let mut p_ratios = Vec::new();
    let mut wasm_ratios = Vec::new();
    for (k, workload) in WORKLOADS.iter().enumerate() {
        let path = freestanding(
            PROGRAMS,
            "bench",
            &format!("bench-{workload}"),
            &[format!("WORKLOAD={}", k + 1), "SCALE=1".to_string()],
        );
        let image = std::fs::read(&path).expect("the built program");
        let mut plain = Vec::new();
        let mut p = Vec::new();
        let mut wasm = Vec::new();
        for _ in 0..rounds {
            let started = Instant::now();
            let output = Command::new(&path).output().expect("the plain run");
            plain.push(started.elapsed().as_secs_f64());
            let started = Instant::now();
            let ours = in_kernel(&mut HostProcessExecutor::new(host()), workload, &image, &[]);
            p.push(started.elapsed().as_secs_f64());
            let started = Instant::now();
            let theirs = in_kernel(&mut web_assembly(), workload, &image, &[]);
            wasm.push(started.elapsed().as_secs_f64());
            assert_eq!(ours.1, output.stdout, "{workload}: form P's output");
            assert_eq!(
                theirs.1, output.stdout,
                "{workload}: the WebAssembly output's"
            );
            let code = output.status.code().expect("the plain run exited");
            assert_eq!(ours.0, code << u8::BITS, "{workload}: form P's status");
            assert_eq!(
                theirs.0,
                code << u8::BITS,
                "{workload}: the WebAssembly output's"
            );
        }
        let (plain, p, wasm) = (median(&mut plain), median(&mut p), median(&mut wasm));
        p_ratios.push(p / plain);
        wasm_ratios.push(wasm / plain);
        println!(
            "{workload}: plain {:.1} ms, form P {:.1} ms ({:.3}), WebAssembly {:.1} ms ({:.3}), medians of {rounds}",
            plain * 1e3,
            p * 1e3,
            p / plain,
            wasm * 1e3,
            wasm / plain
        );
    }
    let geometric =
        |ratios: &[f64]| (ratios.iter().map(|r| r.ln()).sum::<f64>() / ratios.len() as f64).exp();
    println!(
        "geometric mean over the workloads: form P {:.3}, WebAssembly {:.3}; load at the end {}",
        geometric(&p_ratios),
        geometric(&wasm_ratios),
        load().trim()
    );
}

/// busybox's `sh` starting `/bin/true` in a loop, under form P twice on one executor and in the reference, against
/// the model: the reference's own time plus, per program, two process creations and its kernel entries times the round
/// trip, plus the translation time (the first run less the second).
#[test]
#[ignore = "a measurement, run on its own"]
fn busybox_sh_starting_programs() {
    let programs = knob("WCHP_PROGRAMS", 1000);
    let script = format!("i=0; while [ $i -lt {programs} ]; do /bin/true; i=$((i+1)); done");
    let files = image::reference_image();
    let path = on_path("sh");
    let arguments: Vec<&[u8]> = vec![path.as_bytes(), b"-c", script.as_bytes()];
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
    };
    let mut cpu = HostProcessExecutor::new(host());
    let mut runs = Vec::new();
    for _ in 0..2 {
        let mut kernel = Kernel::new();
        image::add_files(&mut kernel, files);
        let entries = cpu.counters().kernel_entries;
        let started = Instant::now();
        let ran = run_container(&mut cpu, kernel, 1, REALTIME_NANOSECONDS, &program);
        runs.push((started.elapsed(), cpu.counters().kernel_entries - entries));
        assert_eq!(
            ran.status,
            0,
            "{:?}",
            String::from_utf8_lossy(&ran.output.stderr)
        );
    }
    // The reference's own time, by busybox's `time` inside the container.
    let timed = format!("time sh -c '{script}'");
    let (outcome, _) = reference::run_command(&["sh", "-c", &timed]);
    let stderr = String::from_utf8_lossy(&outcome.stderr);
    let reference = parse_real(&stderr).unwrap_or_else(|| panic!("no time in {stderr:?}"));
    // A process creation and a round trip, measured here.
    let host = host();
    let creations = 100;
    let started = Instant::now();
    for _ in 0..creations {
        let stub = host.fork_stub().expect("a stub");
        host.kill(&stub).expect("kill");
        host.reap(&stub).expect("reap");
    }
    let creation = started.elapsed() / creations;
    let mut samples = getppid_loop(&mut cpu, 100_000);
    samples.sort_unstable();
    let round_trip = Duration::from_nanos(u64::from(samples[samples.len() / 2]));
    let (first, first_entries) = runs[0];
    let (second, second_entries) = runs[1];
    let translation = first.saturating_sub(second);
    let model = reference
        + creation * 2 * programs as u32
        + round_trip * second_entries as u32
        + translation;
    println!(
        "busybox sh starting {programs} programs: form P {first:?} then {second:?} ({first_entries} and {second_entries} kernel entries), reference {reference:?}; process creation and end {creation:?}, round trip {round_trip:?}, translation {translation:?}; model {model:?}; first run against the model {:.3}; load {}",
        first.as_secs_f64() / model.as_secs_f64(),
        load().trim()
    );
    println!("slow paths: {:?}", cpu.slow_paths());
}

/// The real time busybox's `time` prints: "real\t0m 1.23s".
fn parse_real(text: &str) -> Option<Duration> {
    let line = text.lines().find(|line| line.starts_with("real"))?;
    let value = line.split_whitespace().skip(1).collect::<String>();
    let (minutes, seconds) = value.trim_end_matches('s').split_once('m')?;
    let minutes: f64 = minutes.parse().ok()?;
    let seconds: f64 = seconds.parse().ok()?;
    Some(Duration::from_secs_f64(minutes * 60.0 + seconds))
}

/// The getppid program of the kernel test in the kernel's loop under form P: whole runs of many calls against none,
/// the mean per call with the kernel's own share.
#[test]
#[ignore = "a measurement, run on its own"]
fn getppid_in_the_kernel() {
    let calls = knob("WCHP_CALLS", 1_000_000);
    let image = compile("../hardware-virtualization/tests/guest", "getppid", true);
    let mut with = Vec::new();
    let mut without = Vec::new();
    for _ in 0..knob("WCHP_ROUNDS", 5) {
        for (count, into) in [(calls, &mut with), (0, &mut without)] {
            let started = Instant::now();
            let (status, _) = in_kernel(
                &mut HostProcessExecutor::new(host()),
                "getppid",
                &image,
                &[&count.to_string()],
            );
            into.push(started.elapsed().as_secs_f64());
            assert_eq!(status, 0);
        }
    }
    let per_call = (median(&mut with) - median(&mut without)) / calls as f64;
    println!(
        "getppid in the kernel's loop under form P: {:.0} ns per call over {calls} calls (median runs), load {}",
        per_call * 1e9,
        load().trim()
    );
}
