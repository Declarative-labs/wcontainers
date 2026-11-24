//! Form P's speed figures (plan 13's W1 bar), each a measurement run on its own with `--ignored --nocapture`:
//! - the guest's getppid round trip, translated code calling the system call's floor entry and answered by the
//!   test's loop at once, timed at every exit (knobs `WCHP_CALLS`, `WCHP_PER_BATCH`);
//! - bench.c's eight workloads run plainly as host programs, under form P and under the WebAssembly output, each in
//!   the kernel's loop with the seeded test policy, in alternating rounds (knob `WCHP_ROUNDS`), outputs checked equal;
//! - placement: the getppid loop with work between calls after a move off a contended CPU against a free one, and the
//!   moves with every CPU of the set contended (knobs `WCHP_CALLS`, `WCHP_WORK`, `WCHP_SET`, `WCHP_SECONDS`);
//! - busybox `sh` starting programs in a loop (knob `WCHP_PROGRAMS`), twice on one executor, the second reusing the
//!   first's translations, against the reference's own time in Docker and the model of plan 13.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

mod guest_build;
#[expect(
    dead_code,
    reason = "the policy's turns are compared by the kernel test alone"
)]
mod kernel_host;

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use guest_build::{PROGRAMS, compile, freestanding};
use kernel_host::{Program, REALTIME_NANOSECONDS, run_container};
use linux_raw_sys::general::{S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH};
use wcontainers_abi::{Cpu, Exit, MemoryId, Permissions, RegisterBlock, Resume, ThreadId, Warning};
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
    let (memory, thread) = loop_guest(cpu, 0);
    let samples = loop_calls(cpu, thread, calls);
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    samples
}

/// The register the work between calls counts down in.
const WORK: u8 = 10;

/// A memory holding the getppid loop with `work` turns of a loop of a subtraction and a branch between calls, and its
/// thread.
fn loop_guest(cpu: &mut HostProcessExecutor, work: u16) -> (MemoryId, ThreadId) {
    let mut words = Vec::new();
    if work > 0 {
        words.extend([
            encode::movz(WORK, work, 0),
            encode::add_immediate(WORK, WORK, -1).expect("an add"),
            encode::compare_branch(true, WORK, -INSTRUCTION).expect("cbnz"),
        ]);
    }
    words.extend([
        encode::movz(SYSCALL_NUMBER, GETPPID, 0),
        encode::exception(form::SVC, 0),
        encode::add_immediate(COUNTER, COUNTER, -1).expect("an add"),
    ]);
    let back = -(words.len() as i64) * INSTRUCTION;
    words.extend([
        encode::compare_branch(true, COUNTER, back).expect("cbnz"),
        encode::movz(SYSCALL_NUMBER, EXIT, 0),
        encode::exception(form::SVC, 0),
    ]);
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let memory = cpu.memory_new().expect("memory");
    cpu.set_permissions(memory, CODE, PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code");
    cpu.write(memory, CODE, &bytes).expect("the code");
    cpu.code_changed(memory, CODE, PAGE, &[CODE]);
    let thread = cpu.thread_new(memory).expect("thread");
    (memory, thread)
}

/// `calls` getppid calls of the loop `thread` runs, from its start, every exit timed.
fn loop_calls(cpu: &mut HostProcessExecutor, thread: ThreadId, calls: u64) -> Vec<u32> {
    let mut registers = RegisterBlock::ZERO;
    registers.pc = CODE;
    registers.x[usize::from(COUNTER)] = calls;
    cpu.set_registers(thread, &registers);
    let mut timed = Timed {
        last: Instant::now(),
        samples: Vec::with_capacity(calls as usize),
    };
    let Ok(()) = cpu.run(thread, Resume::FromRegisters, Turn::Free, &mut timed);
    assert_eq!(timed.samples.len() as u64, calls, "every call came back");
    timed.samples
}

/// The median of the gaps between calls, the first (which holds the turn's start) left out.
fn median_gap(samples: &[u32]) -> f64 {
    let mut gaps: Vec<f64> = samples[1..].iter().map(|&gap| f64::from(gap)).collect();
    median(&mut gaps)
}

/// The calls of each loop the placement measurements run, and the work between them (about 10 us on Graviton4).
fn placement_knobs() -> (u64, u16) {
    (
        knob("WCHP_CALLS", 20_000),
        u16::try_from(knob("WCHP_WORK", 30_000)).expect("WCHP_WORK fits 16 bits"),
    )
}

/// Pins the calling thread to `cpus`.
fn pin_to(cpus: &[usize]) {
    let mut set = rustix::thread::CpuSet::new();
    for &cpu in cpus {
        set.set(cpu);
    }
    rustix::thread::sched_setaffinity(None, &set).expect("pin");
}

/// A thread spinning on `cpu` until `spinning` is cleared.
fn spinner(cpu: usize, spinning: &Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    let spinning = spinning.clone();
    std::thread::spawn(move || {
        pin_to(&[cpu]);
        while spinning.load(Ordering::Relaxed) {
            std::hint::spin_loop();
        }
    })
}

/// Plan 13's placement item: with a program spinning on the runner's CPU, the runner moves once and the guest's getppid
/// loop with work between calls runs within 5% of the same loop on a free CPU, both measured here (knobs `WCHP_CALLS`,
/// `WCHP_WORK`).
#[test]
#[ignore = "a measurement, run on its own"]
fn placement_after_a_move_runs_as_on_a_free_cpu() {
    let (calls, work) = placement_knobs();
    let mut cpu = HostProcessExecutor::new(host());
    let (memory, thread) = loop_guest(&mut cpu, work);
    loop_calls(&mut cpu, thread, calls / 10);
    let free = median_gap(&loop_calls(&mut cpu, thread, calls));
    let first = cpu.runner_cpu().expect("placed");
    let before = cpu.counters().placement_moves;
    let spinning = Arc::new(AtomicBool::new(true));
    let other = spinner(first, &spinning);
    let started = Instant::now();
    let mut loops = 0;
    while cpu.counters().placement_moves == before && started.elapsed() < Duration::from_secs(10) {
        loop_calls(&mut cpu, thread, calls / 100);
        loops += 1;
    }
    let took = started.elapsed();
    let moved = cpu.runner_cpu().expect("placed");
    let after = median_gap(&loop_calls(&mut cpu, thread, calls));
    let moves = cpu.counters().placement_moves - before;
    spinning.store(false, Ordering::Relaxed);
    other.join().expect("the spinner");
    // The same loop on the CPU it moved to, now free too, as the measure the move is held to.
    let free_there = median_gap(&loop_calls(&mut cpu, thread, calls));
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    println!(
        "placement: free CPU {first} {free:.0} ns a call; a program spinning there, moved to {moved} after {loops} \
         loops of {} calls ({took:?}), {moves} moves; then {after:.0} ns a call; with the program ended \
         {free_there:.0} ns there; {:.4} times that, {:.4} times the first; load {}",
        calls / 100,
        after / free_there,
        after / free,
        load().trim()
    );
    assert_eq!(moves, 1, "one move");
    assert!(
        after <= free_there * 1.05,
        "{after:.0} ns against {free_there:.0} ns"
    );
}

/// Plan 13's placement item: with every CPU of the embedder's set contended by a spinning program, the runner moves at
/// most once a second (knobs `WCHP_SET`, the set's CPUs, the first the process may use, default 4; `WCHP_SECONDS`,
/// default 10; `WCHP_CALLS`, `WCHP_WORK`).
#[test]
#[ignore = "a measurement, run on its own"]
fn placement_with_every_cpu_contended_moves_at_most_once_a_second() {
    let (calls, work) = placement_knobs();
    let seconds = knob("WCHP_SECONDS", 10);
    let allowed = rustix::thread::sched_getaffinity(None).expect("affinity");
    let set: Vec<usize> = (0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|&cpu| allowed.is_set(cpu))
        .take(knob("WCHP_SET", 4) as usize)
        .collect();
    // The host reads the CPUs it may use from the thread that makes it.
    pin_to(&set);
    let host = Arc::new(HostProcessHost::new().expect("the host is refused"));
    let spinning = Arc::new(AtomicBool::new(true));
    let others: Vec<_> = set.iter().map(|&cpu| spinner(cpu, &spinning)).collect();
    let mut cpu = HostProcessExecutor::new(host);
    let (memory, thread) = loop_guest(&mut cpu, work);
    let started = Instant::now();
    let mut loops = 0;
    while started.elapsed() < Duration::from_secs(seconds) {
        loop_calls(&mut cpu, thread, calls / 100);
        loops += 1;
    }
    cpu.thread_drop(thread);
    cpu.memory_drop(memory);
    let took = started.elapsed();
    spinning.store(false, Ordering::Relaxed);
    for other in others {
        other.join().expect("a spinner");
    }
    let moves = cpu.counters().placement_moves;
    println!(
        "placement on CPUs {set:?}, each with a program spinning: {moves} moves in {took:?} ({loops} loops of {} \
         calls); load {}",
        calls / 100,
        load().trim()
    );
    assert!(
        moves as f64 <= took.as_secs_f64().ceil(),
        "{moves} moves in {took:?}"
    );
}

/// The bytes of a guest word.
const WORD: u64 = size_of::<u64>() as u64;

/// Where the copy unit measurement's written pages start, and its table of addresses.
const PAGES_AT: u64 = 0x1000_0000;
const TABLE_AT: u64 = 0x800_0000;

/// A fork child's translated stores to its parent's written pages, one store per page: in order over `WCHP_PAGES`
/// pages (default 16,384), then to every `WCHP_STRIDE`th page (default 16) in a seeded order (`WCHP_SEED`); each the
/// time a store, the pages copied and the child's host mappings, for the copy unit (plan 3.5).
#[test]
#[ignore = "a measurement, run on its own"]
fn copy_unit_on_translated_stores() {
    let pages = knob("WCHP_PAGES", 16_384);
    let stride = knob("WCHP_STRIDE", 16);
    let mut random = fastrand::Rng::with_seed(knob("WCHP_SEED", 1));
    let (address, value, left, table) = (5, 6, 2, 4);
    let words = [
        encode::load_store(form::LDR, address, table, 0, WORD).expect("ldr"),
        encode::add_immediate(table, table, WORD as i64).expect("an add"),
        encode::load_store(form::STR, value, address, 0, WORD).expect("str"),
        encode::add_immediate(left, left, -1).expect("an add"),
        encode::compare_branch(true, left, -4 * INSTRUCTION).expect("cbnz"),
        encode::movz(SYSCALL_NUMBER, EXIT, 0),
        encode::exception(form::SVC, 0),
    ];
    let code: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut cpu = HostProcessExecutor::new(host());
    let parent = cpu.memory_new().expect("memory");
    cpu.set_permissions(parent, CODE, PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code");
    cpu.write(parent, CODE, &code).expect("the code");
    cpu.code_changed(parent, CODE, PAGE, &[CODE]);
    let rw = Permissions::READ | Permissions::WRITE;
    cpu.set_permissions(parent, PAGES_AT, pages * PAGE, rw)
        .expect("pages");
    for index in 0..pages {
        cpu.write(parent, PAGES_AT + index * PAGE, &index.to_le_bytes())
            .expect("a page");
    }
    let mut scattered: Vec<u64> = (0..pages).step_by(stride as usize).collect();
    random.shuffle(&mut scattered);
    for (name, order) in [
        ("in order", (0..pages).collect::<Vec<_>>()),
        ("scattered", scattered),
    ] {
        let addresses: Vec<u8> = order
            .iter()
            .flat_map(|index| (PAGES_AT + index * PAGE).to_le_bytes())
            .collect();
        let table_pages = (addresses.len() as u64).div_ceil(PAGE);
        cpu.set_permissions(parent, TABLE_AT, table_pages * PAGE, Permissions::READ)
            .expect("table");
        cpu.write(parent, TABLE_AT, &addresses).expect("the table");
        let child = cpu.memory_duplicate(parent).expect("fork");
        let thread = cpu.thread_new(child).expect("thread");
        let mut registers = RegisterBlock::ZERO;
        registers.pc = CODE;
        registers.x[usize::from(table)] = TABLE_AT;
        registers.x[usize::from(left)] = order.len() as u64;
        registers.x[usize::from(value)] = 1;
        cpu.set_registers(thread, &registers);
        let copied = cpu.counters().pages_copied;
        let mut timed = Timed {
            last: Instant::now(),
            samples: Vec::new(),
        };
        let started = Instant::now();
        let Ok(()) = cpu.run(thread, Resume::FromRegisters, Turn::Free, &mut timed);
        let took = started.elapsed();
        for (at, index) in [order[0], order[order.len() - 1]]
            .into_iter()
            .map(|index| (PAGES_AT + index * PAGE, index))
        {
            let mut word = [0; 8];
            cpu.read(child, at, &mut word).expect("read");
            assert_eq!(
                u64::from_le_bytes(word),
                1,
                "the child's store to page {index}"
            );
            cpu.read(parent, at, &mut word).expect("read");
            assert_eq!(u64::from_le_bytes(word), index, "the parent's page {index}");
        }
        println!(
            "copy unit, {name}: {} stores, {:.2} us each, {} pages copied, host mappings child {}; load {}",
            order.len(),
            took.as_secs_f64() * 1e6 / order.len() as f64,
            cpu.counters().pages_copied - copied,
            cpu.host_mappings(child),
            load().trim()
        );
        cpu.thread_drop(thread);
        cpu.memory_drop(child);
    }
    cpu.memory_drop(parent);
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
        limit: None,
        turn_events: None,
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
        limit: None,
        turn_events: None,
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
