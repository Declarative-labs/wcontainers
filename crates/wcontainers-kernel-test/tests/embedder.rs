//! The embedder's interface: the life-cycle hooks and their order, task ids handed out by the
//! embedder, watches met before a call has any effect, the record of written pages, and lazy
//! memory built from the two primitives running the shell's scripts as the reference does.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use std::ops::ControlFlow;

use wcontainers_abi::{Cpu, PAGE_SIZE, SystemCallRequest};
use wcontainers_kernel::linux::general;
use wcontainers_kernel::{
    AccessKind, ContextOrigin, Errno, ExecutionConfig, Kernel, MemoryError, MemoryObject, Process,
    SignalInformation, SuspendedTask, System, SystemCallResponse, SystemVSegmentId, TaskId,
    WatchesFor, reason, task_id_after,
};
use wcontainers_kernel_test::driver::{self, Run};
use wcontainers_kernel_test::hooks::Hooks;
use wcontainers_kernel_test::reference::{self, ROOT_DIRECTORY, in_guest_directory};
use wcontainers_kernel_test::{busybox, guest_directory_entry};

fn out() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("embedder")
}

/// Where embedder.c is told to map its buffer, and how many pages.
const BUFFER: u64 = 0x3700_0000;
const PAGES: u64 = 8;

fn buffer() -> std::ops::Range<u64> {
    BUFFER..BUFFER + PAGES * PAGE_SIZE
}

fn page_of(address: u64) -> std::ops::Range<u64> {
    let start = address / PAGE_SIZE * PAGE_SIZE;
    start..start + PAGE_SIZE
}

/// Runs embedder.c in `mode` on our kernel with the test's hooks.
fn run_embedder(mode: &str, hooks: Box<dyn Hooks>, seed: u64) -> Run {
    run_embedder_below(&[mode], Some(hooks), seed, None)
}

/// As [`run_embedder`], with more arguments, the executor's ceiling given at start, and with no
/// hooks the kernel's provided defaults.
fn run_embedder_below(
    mode: &[&str],
    hooks: Option<Box<dyn Hooks>>,
    seed: u64,
    ceiling: Option<u64>,
) -> Run {
    try_embedder_below(mode, hooks, seed, ceiling, BUFFER).expect("an outcome on our kernel")
}

/// As [`run_embedder_below`], with the buffer at `buffer` and a failure to start given back.
fn try_embedder_below(
    mode: &[&str],
    hooks: Option<Box<dyn Hooks>>,
    seed: u64,
    ceiling: Option<u64>,
    buffer: u64,
) -> Result<Run, driver::Failure> {
    let (buffer, pages) = (format!("{buffer:#x}"), PAGES.to_string());
    let mut arguments = vec![mode[0], buffer.as_str(), pages.as_str()];
    arguments.extend(&mode[1..]);
    try_guest("embedder", &arguments, hooks, seed, ceiling)
}

/// Runs guest program `name` with `arguments` on our kernel, with `hooks` if given.
fn try_guest(
    name: &str,
    arguments: &[&str],
    hooks: Option<Box<dyn Hooks>>,
    seed: u64,
    ceiling: Option<u64>,
) -> Result<Run, driver::Failure> {
    let program = wcontainers_kernel_test::guest::build(name, &[], &out());
    let mut files = wcontainers_kernel_test::image::reference_image().to_vec();
    files.extend(wcontainers_kernel_test::image::host_directory(
        program.parent().expect("a directory"),
        guest_directory_entry(),
    ));
    let path = in_guest_directory(name);
    let mut command = vec![path.as_str()];
    command.extend(arguments);
    let container = driver::Container {
        files: &files,
        path: &path,
        arguments: &command,
        environment: reference::ENVIRONMENT,
        working_directory: ROOT_DIRECTORY,
        backend: None,
        cached: false,
        published: &[],
        address_space: wcontainers_kernel_test::standin::SPACE_32_BIT,
    };
    let prepared = driver::Prepared {
        hooks,
        ceiling,
        ..driver::prepare(&container)
    };
    driver::run_prepared(prepared, &container, seed, Default::default())
}

fn stdout(run: &Run) -> String {
    String::from_utf8_lossy(&run.outcome.stdout).into_owned()
}

fn counted(run: &Run, which: &str) -> u64 {
    run.counters
        .iter()
        .filter(|counter| counter.reason == which)
        .map(|counter| counter.count)
        .sum()
}

/// One hook or task id call as a test saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Seen {
    Starting { id: u32, parent: Option<u32> },
    Program(u32),
    Thread { id: u32, thread: u32 },
    Exiting(u32),
    Free(u32),
}

/// Records every hook in order, uses every operation of the handle inside each to try to make a
/// hook nest, and with `pool` hands out its own task ids, reusing freed ones first.
#[derive(Default)]
struct Recorder {
    seen: Arc<Mutex<Vec<Seen>>>,
    depth: Arc<Mutex<u32>>,
    pool: Option<(u32, Vec<u32>)>,
    allocated: Arc<Mutex<Vec<u32>>>,
}

impl Recorder {
    /// Enters a hook: none may be running already. Every operation of the handle runs, and
    /// none may announce another hook.
    fn enter(&self, process: &mut Process<'_>, seen: Seen) {
        self.operate(process, &format!("{seen:?}"));
        self.seen.lock().unwrap().push(seen);
    }

    /// Runs every operation of the handle inside a hook, which no other hook may surround.
    fn operate(&self, process: &mut Process<'_>, seen: &str) {
        assert_eq!(
            *self.depth.lock().unwrap(),
            0,
            "a hook nested in another: {seen}"
        );
        *self.depth.lock().unwrap() += 1;
        let info = process.info();
        assert!(info.memory_mapped() > 0, "{seen}: memory mapped");
        let _ = (info.ceiling_distance(), info.compute_used());
        let _ = process.execution_config();
        let changed = process.take_dirty_memory();
        if let Some(first) = changed.first() {
            let bytes = process
                .read_memory(first.start..first.start + 1)
                .expect("readable");
            // A page of a shared mapping its process cannot write is refused, not written.
            let written = process.write_memory(first.start, &bytes);
            assert!(
                matches!(written, Ok(()) | Err(MemoryError::SharedNotWritable)),
                "{seen}: {written:?}"
            );
            process.watch_memory(first.clone(), WatchesFor::None);
        }
        assert!(
            process
                .watch_thread_memory(TaskId(0), buffer(), WatchesFor::All)
                .is_err()
        );
        // Ranges that end before they start or past the last address are refused, not obeyed.
        let reversed = std::ops::Range {
            start: BUFFER + PAGE_SIZE,
            end: BUFFER,
        };
        assert_eq!(
            process.read_memory(reversed.clone()),
            Err(MemoryError::InvalidRange)
        );
        assert_eq!(
            process.write_memory(u64::MAX, &[0, 0]),
            Err(MemoryError::InvalidRange)
        );
        process.watch_memory(reversed, WatchesFor::All);
        process.watch_memory(u64::MAX - 1..u64::MAX, WatchesFor::All);
        let _ = process.memory_id();
        let copy = process.copy_process_context();
        assert_eq!(
            (copy.id(), copy.origin()),
            (process.id(), ContextOrigin::Copied),
            "{seen}"
        );
        // No task asked for: none taken. Taking the process itself is the suspend tests' part.
        assert_eq!(process.suspend_tasks(&[]).count(), 0, "{seen}");
        // A signal of number 0 is counted and not sent.
        process.send_signal(process.id(), queued_signal(0, 0));
        *self.depth.lock().unwrap() -= 1;
    }
}

impl Hooks for Recorder {
    fn allocate_task_id(&mut self, previous: TaskId) -> TaskId {
        let Some((next, freed)) = &mut self.pool else {
            return wcontainers_kernel::task_id_after(previous);
        };
        let id = freed.pop().unwrap_or_else(|| {
            *next += 1;
            *next
        });
        self.allocated.lock().unwrap().push(id);
        TaskId(id)
    }

    fn free_task_id(&mut self, id: TaskId) {
        assert_eq!(
            *self.depth.lock().unwrap(),
            0,
            "a task id freed inside a hook"
        );
        if let Some((_, freed)) = &mut self.pool {
            assert!(!freed.contains(&id.0), "{id:?} freed twice");
            freed.push(id.0);
        }
        self.seen.lock().unwrap().push(Seen::Free(id.0));
    }

    fn intercept_system_call_request(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        self.operate(&mut process, &format!("request {request:?}"));
        ControlFlow::Continue(request)
    }

    fn intercept_system_call_response(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        self.operate(&mut process, &format!("response {request:?}"));
        response
    }

    fn on_memory_written_by_kernel(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        range: std::ops::Range<u64>,
    ) {
        self.operate(&mut process, &format!("written {range:?}"));
    }

    fn on_signal_delivering(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        signal: &SignalInformation,
    ) {
        let (number, ..) = signal_fields(signal);
        self.operate(&mut process, &format!("signal {number} to {thread:?}"));
    }

    fn on_shared_memory_mapping(
        &mut self,
        mut process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: std::ops::Range<u64>,
    ) {
        self.operate(
            &mut process,
            &format!("mapping {source:?} {offset} {range:?}"),
        );
    }

    fn on_shared_memory_unmapped(
        &mut self,
        mut process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: std::ops::Range<u64>,
    ) {
        self.operate(
            &mut process,
            &format!("unmapped {source:?} {offset} {range:?}"),
        );
    }

    fn on_process_starting(&mut self, mut process: Process<'_>) {
        let seen = Seen::Starting {
            id: process.id().0,
            parent: process.info().parent().map(|parent| parent.0),
        };
        self.enter(&mut process, seen);
    }

    fn on_program_starting(&mut self, mut process: Process<'_>) {
        let seen = Seen::Program(process.id().0);
        self.enter(&mut process, seen);
    }

    fn on_thread_starting(&mut self, mut process: Process<'_>, thread: TaskId) {
        let seen = Seen::Thread {
            id: process.id().0,
            thread: thread.0,
        };
        self.enter(&mut process, seen);
    }

    fn on_process_exiting(&mut self, mut process: Process<'_>) {
        let seen = Seen::Exiting(process.id().0);
        self.enter(&mut process, seen);
    }
}

/// The order tests' check that no hook nests can fail: a call into the test's hooks while one is
/// running is reported, not waited on.
#[test]
#[should_panic(expected = "a hook nested in another")]
fn a_hook_called_while_one_runs_is_reported_as_nested() {
    let host = driver::TestHost::new(1).with_hooks(Box::new(Recorder::default()));
    let _running = host.hook();
    host.allocate_task_id(TaskId(1));
}

#[test]
fn life_cycle_hooks_fire_in_order_over_fork_exec_threads_and_exit() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let recorder = Recorder::default();
        let seen = recorder.seen.clone();
        let run = run_embedder("lifecycle", Box::new(recorder), seed);
        assert_eq!(
            stdout(&run),
            "child 0\n",
            "{:?} {:?} {:?}",
            run.outcome,
            run.counters,
            seen.lock().unwrap()
        );
        // The first process (1), its thread (2), whose number goes back once it has ended, then
        // a child (3) that runs a program, ends, and is reaped; then the first process ends and
        // with the container nothing carries its number.
        let expected = [
            Seen::Starting {
                id: 1,
                parent: None,
            },
            Seen::Program(1),
            Seen::Thread { id: 1, thread: 2 },
            Seen::Free(2),
            Seen::Starting {
                id: 3,
                parent: Some(1),
            },
            Seen::Program(3),
            Seen::Exiting(3),
            Seen::Free(3),
            Seen::Exiting(1),
            Seen::Free(1),
        ];
        assert_eq!(*seen.lock().unwrap(), expected, "seed {seed}");
    }
}

#[test]
fn task_ids_come_from_the_embedder_and_go_back_once_nothing_carries_them() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let recorder = Recorder {
            pool: Some((1000, Vec::new())),
            ..Default::default()
        };
        let (seen, allocated) = (recorder.seen.clone(), recorder.allocated.clone());
        let run = run_embedder("lifecycle", Box::new(recorder), seed);
        assert_eq!(stdout(&run), "child 0\n", "{:?}", run.outcome);
        // The thread's number, freed when it ended, is the child's.
        assert_eq!(
            *allocated.lock().unwrap(),
            [1001, 1002, 1002],
            "seed {seed}"
        );
        let expected = [
            Seen::Starting {
                id: 1001,
                parent: None,
            },
            Seen::Program(1001),
            Seen::Thread {
                id: 1001,
                thread: 1002,
            },
            Seen::Free(1002),
            Seen::Starting {
                id: 1002,
                parent: Some(1001),
            },
            Seen::Program(1002),
            Seen::Exiting(1002),
            Seen::Free(1002),
            Seen::Exiting(1001),
            Seen::Free(1001),
        ];
        assert_eq!(*seen.lock().unwrap(), expected, "seed {seed}");
    }
}

/// Records each `on_process_outgrown`: the bytes asked for and the distance left to the ceiling.
struct Outgrown(Arc<Mutex<Vec<(u64, u64)>>>);

impl Hooks for Outgrown {
    fn on_process_outgrown(&mut self, process: Process<'_>, bytes: u64) {
        self.0
            .lock()
            .unwrap()
            .push((bytes, process.info().ceiling_distance()));
    }
}

/// The piece embedder.c's `outgrow` is told to map at a time, and the ceiling the test gives.
const PIECE: u64 = 64 << 20;
const CEILING: u64 = 1 << 30;

#[test]
fn a_request_past_the_ceiling_is_announced_and_fails_as_out_of_memory() {
    let piece = format!("{PIECE:#x}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let run = run_embedder_below(
            &["outgrow", &piece],
            Some(Box::new(Outgrown(seen.clone()))),
            seed,
            Some(CEILING),
        );
        let text = stdout(&run);
        let words: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(words[0], "outgrown", "{text}");
        assert_eq!(words[2], "ENOMEM", "{text}");
        let pieces: u64 = words[1].parse().expect("a count");
        assert!(pieces > 0 && pieces * PIECE < CEILING, "{text}");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "{seen:?}");
        let (bytes, distance) = seen[0];
        assert_eq!(bytes, PIECE);
        assert!(distance < PIECE, "{distance}");
        assert_eq!(counted(&run, reason::PROCESS_OUTGROWN), 1);
        // With no ceiling below the address space's end, nothing is announced.
        let seen_without = Arc::new(Mutex::new(Vec::new()));
        let run = run_embedder_below(
            &["outgrow", &piece],
            Some(Box::new(Outgrown(seen_without.clone()))),
            seed,
            None,
        );
        assert!(seen_without.lock().unwrap().is_empty());
        let more: u64 = stdout(&run)
            .split_whitespace()
            .nth(1)
            .expect("a count")
            .parse()
            .expect("a count");
        assert!(more > pieces, "{}", stdout(&run));
    }
}

#[test]
fn a_fixed_request_past_the_ceiling_is_announced_and_fails_as_out_of_memory() {
    let address = format!("{:#x}", CEILING + CEILING / 4);
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let run = run_embedder_below(
            &["fixed", &address],
            Some(Box::new(Outgrown(seen.clone()))),
            seed,
            Some(CEILING),
        );
        let text = stdout(&run);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(
            lines.iter().all(|line| !line.ends_with(" mapped")),
            "{text}"
        );
        assert_eq!(
            seen.lock()
                .unwrap()
                .iter()
                .map(|(bytes, _)| *bytes)
                .collect::<Vec<_>>(),
            [PAGE_SIZE, PAGE_SIZE]
        );
        assert_eq!(counted(&run, reason::PROCESS_OUTGROWN), 2);
        assert_eq!(counted(&run, reason::ADDRESS_ABOVE_SPACE), 0);
        // Below the address space's end and with no ceiling, both are mapped, as on Linux.
        let run = run_embedder_below(&["fixed", &address], None, seed, None);
        assert_eq!(stdout(&run), "fixed mapped\nnoreplace mapped\n");
    }
}

#[test]
fn heap_growth_past_the_ceiling_is_announced_and_fails_as_out_of_memory() {
    let growth = format!("{CEILING:#x}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let run = run_embedder_below(
            &["brk", &growth],
            Some(Box::new(Outgrown(seen.clone()))),
            seed,
            Some(CEILING),
        );
        assert_eq!(stdout(&run), "brk refused\n", "{:?}", run.outcome);
        assert_eq!(
            seen.lock()
                .unwrap()
                .iter()
                .map(|(bytes, _)| *bytes)
                .collect::<Vec<_>>(),
            [CEILING]
        );
        assert_eq!(counted(&run, reason::PROCESS_OUTGROWN), 1);
        // With no ceiling below the address space's end the heap grows, as on Linux.
        let run = run_embedder_below(&["brk", &growth], None, seed, None);
        assert_eq!(stdout(&run), "brk grown\n", "{:?}", run.outcome);
    }
}

/// A ceiling below the least gap Linux leaves under the stack's top for the mmap base, a buffer
/// below it and above the program, the piece mapped at a time under it, and a ceiling too low for the program itself.
const SMALL_CEILING: u64 = 100 << 20;
const LOW_BUFFER: u64 = 48 << 20;
const SMALL_PIECE: u64 = 4 << 20;
const TINY_CEILING: u64 = 16 * PAGE_SIZE;

#[test]
fn a_small_ceiling_is_kept_and_a_first_program_past_it_is_refused() {
    let piece = format!("{SMALL_PIECE:#x}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let run = try_embedder_below(
            &["outgrow", &piece],
            Some(Box::new(Outgrown(seen.clone()))),
            seed,
            Some(SMALL_CEILING),
            LOW_BUFFER,
        )
        .expect("an outcome on our kernel");
        let text = stdout(&run);
        let words: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(words.first(), Some(&"outgrown"), "{:?}", run.outcome);
        assert_eq!(words[2], "ENOMEM", "{text}");
        let pieces: u64 = words[1].parse().expect("a count");
        assert!(pieces > 0 && pieces * SMALL_PIECE < SMALL_CEILING, "{text}");
        let sizes: Vec<u64> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|(bytes, _)| *bytes)
            .collect();
        assert_eq!(sizes, [SMALL_PIECE]);
        assert_eq!(counted(&run, reason::PROCESS_OUTGROWN), 1);
        // The first program does not fit: its start fails as out of memory before it has a
        // process to announce.
        let seen = Arc::new(Mutex::new(Vec::new()));
        let failed = try_embedder_below(
            &["outgrow", &piece],
            Some(Box::new(Outgrown(seen.clone()))),
            seed,
            Some(TINY_CEILING),
            LOW_BUFFER,
        );
        assert!(
            matches!(failed, Err(driver::Failure::Start(error)) if error == Errno::ENOMEM),
            "{:?}",
            failed.map(|run| run.outcome)
        );
        assert!(seen.lock().unwrap().is_empty());
    }
}

#[test]
fn a_new_program_past_the_ceiling_is_refused_before_the_old_one_goes() {
    let program = wcontainers_kernel_test::guest::build("bigdata", &[], &out());
    let path = in_guest_directory(program.file_name().expect("a name").to_str().expect("text"));
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let run = try_embedder_below(
            &["exec", &path],
            Some(Box::new(Outgrown(seen.clone()))),
            seed,
            Some(SMALL_CEILING),
            LOW_BUFFER,
        )
        .expect("an outcome on our kernel");
        // The old program carries on and is told, as Linux's execve refuses before its point of
        // no return.
        assert_eq!(stdout(&run), "exec Out of memory\n", "{:?}", run.outcome);
        assert_eq!(run.outcome.exit_code, 0);
        let sizes: Vec<u64> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|(bytes, _)| *bytes)
            .collect();
        assert_eq!(sizes.len(), 1, "{sizes:?}");
        assert!(sizes[0] > SMALL_CEILING, "{sizes:?}");
        assert_eq!(counted(&run, reason::PROCESS_OUTGROWN), 1);
        // With no ceiling below the address space's end it runs.
        let run = try_embedder_below(&["exec", &path], None, seed, None, LOW_BUFFER)
            .expect("an outcome on our kernel");
        assert_eq!((stdout(&run).as_str(), run.outcome.exit_code), ("", 0));
    }
}

/// The default counter's numbers, but `answer` on the `at`-th ask.
struct Misnumbered {
    asked: u32,
    at: u32,
    answer: TaskId,
}

impl Hooks for Misnumbered {
    fn allocate_task_id(&mut self, previous: TaskId) -> TaskId {
        self.asked += 1;
        if self.asked == self.at {
            self.answer
        } else {
            task_id_after(previous)
        }
    }
}

#[test]
fn a_task_id_out_of_range_fails_the_fork_with_eagain() {
    for answer in [TaskId(0), TaskId(u32::MAX)] {
        // The first ask is the first process's, the second the shell's fork.
        let hooks = Misnumbered {
            asked: 0,
            at: 2,
            answer,
        };
        let run = busybox::run_ours_hooked(
            &["sh", "-c", "true & wait $!; echo status $?"],
            1,
            Box::new(hooks),
        );
        assert_eq!(
            String::from_utf8_lossy(&run.outcome.stderr),
            "sh: can't fork: Resource temporarily unavailable\n",
            "{answer:?}"
        );
        assert!(run.outcome.stdout.is_empty(), "{answer:?}");
        assert_eq!(counted(&run, reason::TASK_ID_OUT_OF_RANGE), 1, "{answer:?}");
    }
}

#[test]
fn the_first_process_is_announced_ending_before_the_others_it_takes_along() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let recorder = Recorder::default();
        let seen = recorder.seen.clone();
        let run = run_embedder("orphan", Box::new(recorder), seed);
        assert_eq!(run.outcome.exit_code, 3, "{:?}", run.outcome);
        let expected = [
            Seen::Starting {
                id: 1,
                parent: None,
            },
            Seen::Program(1),
            Seen::Starting {
                id: 2,
                parent: Some(1),
            },
            Seen::Exiting(1),
            Seen::Exiting(2),
            Seen::Free(1),
            Seen::Free(2),
        ];
        assert_eq!(*seen.lock().unwrap(), expected, "seed {seed}");
    }
}

#[test]
fn life_cycle_hooks_fire_in_order_over_signals_threaded_exec_vfork_groups_and_unwanted_children() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let recorder = Recorder::default();
        let seen = recorder.seen.clone();
        let run = run_embedder("paths", Box::new(recorder), seed);
        assert_eq!(
            stdout(&run),
            "signalled 15\nthreaded exec 0\nvfork 0\ngroups left\nunwanted 1\n",
            "{:?}",
            run.outcome
        );
        let starting = |id, parent| Seen::Starting {
            id,
            parent: Some(parent),
        };
        let mut expected = vec![
            Seen::Starting {
                id: 1,
                parent: None,
            },
            Seen::Program(1),
            // Killed by a signal, then reaped.
            starting(2, 1),
            Seen::Exiting(2),
            Seen::Free(2),
            // Its second thread runs a program and takes the process's number; its own goes.
            starting(3, 1),
            Seen::Thread { id: 3, thread: 4 },
            Seen::Program(3),
            Seen::Free(4),
            Seen::Exiting(3),
            Seen::Free(3),
            // vfork.
            starting(5, 1),
            Seen::Program(5),
            Seen::Exiting(5),
            Seen::Free(5),
        ];
        // A group leader reaped while its group lives on: its number goes only once the
        // grandchild leaves the group, by setpgid and then by setsid, after a child in between.
        for leader in [6, 9] {
            let (grandchild, between) = (leader + 1, leader + 2);
            expected.extend([
                starting(leader, 1),
                starting(grandchild, leader),
                Seen::Exiting(leader),
                starting(between, 1),
                Seen::Exiting(between),
                Seen::Free(between),
                Seen::Free(leader),
                Seen::Exiting(grandchild),
                Seen::Free(grandchild),
            ]);
        }
        // A child its parent does not want goes as it ends; then the container ends.
        expected.extend([
            starting(12, 1),
            Seen::Exiting(12),
            Seen::Free(12),
            Seen::Exiting(1),
            Seen::Free(1),
        ]);
        assert_eq!(*seen.lock().unwrap(), expected, "seed {seed}");
    }
}

/// What a watching test saw at each hook: the process, the address, how, and whether the
/// watched page still held what it held when the process started.
type Touches = Arc<Mutex<Vec<(u32, u64, AccessKind, bool)>>>;

/// Pages of the buffer a process ended with changed though no hook announced them, by process,
/// with the first changed address and its byte before and after.
type Unannounced = Arc<Mutex<Vec<(u32, u64, u8, u8)>>>;

/// Watches the buffer's writes in every process from its start; meets each watch inside the
/// hook, or `later`, between steps through `Kernel::process`. At each process's end, every page
/// of the buffer whose bytes changed must have been announced in it, or in a child of it (a
/// vfork child writes its parent's memory, which waits; no other child here writes the buffer).
struct Watcher {
    later: bool,
    touches: Touches,
    pending: Vec<(TaskId, u64)>,
    at_start: BTreeMap<u32, Vec<u8>>,
    parents: BTreeMap<u32, u32>,
    announced: BTreeMap<u32, BTreeSet<u64>>,
    unannounced: Unannounced,
}

impl Watcher {
    fn new(later: bool, touches: Touches, unannounced: Unannounced) -> Self {
        Self {
            later,
            touches,
            pending: Vec::new(),
            at_start: BTreeMap::new(),
            parents: BTreeMap::new(),
            announced: BTreeMap::new(),
            unannounced,
        }
    }
}

impl Hooks for Watcher {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if let Some(parent) = process.info().parent() {
            self.parents.insert(process.id().0, parent.0);
        }
        // The first process maps the buffer only once it runs.
        if let Ok(bytes) = process.read_memory(buffer()) {
            self.at_start.insert(process.id().0, bytes);
            process.watch_memory(buffer(), WatchesFor::Writes);
        }
    }

    fn on_watched_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        access: AccessKind,
    ) {
        let page = page_of(address);
        let now = process
            .read_memory(page.clone())
            .expect("the buffer is mapped");
        let offset = (page.start - BUFFER) as usize;
        let unchanged = self.at_start[&process.id().0][offset..offset + now.len()] == now[..];
        self.touches
            .lock()
            .unwrap()
            .push((process.id().0, address, access, unchanged));
        let id = process.id().0;
        for announced in [Some(id), self.parents.get(&id).copied()]
            .into_iter()
            .flatten()
        {
            self.announced
                .entry(announced)
                .or_default()
                .insert(page.start);
        }
        if self.later {
            self.pending.push((process.id(), address));
        } else {
            process.watch_memory(page, WatchesFor::None);
        }
    }

    fn on_process_exiting(&mut self, mut process: Process<'_>) {
        let id = process.id().0;
        let (Some(start), Ok(end)) = (self.at_start.get(&id), process.read_memory(buffer())) else {
            return;
        };
        let announced = self.announced.remove(&id).unwrap_or_default();
        for (index, (before, after)) in start
            .chunks(PAGE_SIZE as usize)
            .zip(end.chunks(PAGE_SIZE as usize))
            .enumerate()
        {
            let page = BUFFER + index as u64 * PAGE_SIZE;
            let first = before.iter().zip(after).position(|(old, new)| old != new);
            if let Some(at) = first
                && !announced.contains(&page)
            {
                let address = page + at as u64;
                (self.unannounced.lock().unwrap()).push((id, address, before[at], after[at]));
            }
        }
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let pending = std::mem::take(&mut self.pending);
        for (id, address) in &pending {
            let mut process = kernel.process(cpu, *id).expect("the process waits");
            process.watch_memory(page_of(*address), WatchesFor::None);
        }
        !pending.is_empty()
    }
}

/// How many calls embedder.c's `calls` makes, from its first line.
fn calls_made(run: &Run) -> usize {
    let text = stdout(run);
    let first = text.lines().next().unwrap_or_default();
    first
        .strip_prefix("calls ")
        .and_then(|count| count.parse().ok())
        .unwrap_or_else(|| panic!("no count of calls: {text}"))
}

#[test]
fn a_watch_is_met_before_a_call_writes_and_the_result_is_unchanged() {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let theirs = reference::run(&program, &["calls", &address, &pages]);
    let theirs = String::from_utf8_lossy(&theirs.stdout);
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder_below(&["calls"], None, seed, None);
        assert_eq!(plain.outcome.exit_code, 0, "{:?}", plain.outcome);
        assert_eq!(stdout(&plain), theirs, "seed {seed}");
        let calls = calls_made(&plain);
        assert_eq!(
            stdout(&plain).lines().count(),
            calls + 2,
            "{}",
            stdout(&plain)
        );
        for later in [false, true] {
            let touches = Touches::default();
            let unannounced = Unannounced::default();
            let watcher = Watcher::new(later, touches.clone(), unannounced.clone());
            let run = run_embedder("calls", Box::new(watcher), seed);
            assert_eq!(stdout(&run), stdout(&plain), "seed {seed}, later {later}");
            assert!(
                unannounced.lock().unwrap().is_empty(),
                "seed {seed}, later {later}: changed with no hook: {:?}, hooks {:?}, output {}",
                unannounced.lock().unwrap(),
                touches.lock().unwrap(),
                stdout(&run)
            );
            let touches = touches.lock().unwrap();
            let processes: BTreeSet<u32> = touches.iter().map(|touch| touch.0).collect();
            assert_eq!(
                processes.len(),
                calls,
                "seed {seed}, later {later}: {touches:?}"
            );
            for (id, address, access, unchanged) in touches.iter() {
                assert!(buffer().contains(address), "{id}: {address:#x}");
                assert_eq!(*access, AccessKind::Write);
                assert!(
                    unchanged,
                    "{id}: the page of {address:#x} changed before the hook: {touches:?} {}",
                    stdout(&run)
                );
            }
            for counter in [
                reason::COPY_UNDESCRIBED,
                reason::WATCH_MET_AT_A_COPY,
                reason::CALL_SHORT_OF_WATCHED_MEMORY,
            ] {
                assert_eq!(counted(&run, counter), 0, "{counter}: {:?}", run.counters);
            }
        }
    }
}

#[test]
fn an_access_that_ends_before_a_watched_page_does_not_announce_it() {
    // span's store: the last 16 bytes but 8 of the buffer's second page.
    let store = BUFFER + 2 * PAGE_SIZE - 2 * size_of::<u64>() as u64;
    for seed in wcontainers_kernel_test::schedule_seeds() {
        // The watch on the third page is in force: the same watch on the store's own page fires.
        for (page, expected) in [(1, vec![store]), (2, vec![])] {
            let touches = Touches::default();
            let run = run_embedder("span", Box::new(NextPage(page, touches.clone())), seed);
            assert_eq!(run.outcome.exit_code, 0, "{:?}", run.outcome);
            let touches = touches.lock().unwrap();
            let addresses: Vec<u64> = touches.iter().map(|touch| touch.1).collect();
            assert_eq!(addresses, expected, "seed {seed}, page {page}: {touches:?}");
            assert!(touches.iter().all(|touch| touch.2 == AccessKind::Write));
        }
    }
}

/// Keeps a record of written pages and watches one page of the buffer, by index, in every child.
struct NextPage(u64, Touches);

impl Hooks for NextPage {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            process.take_dirty_memory();
            process.watch_memory(page_of(BUFFER + self.0 * PAGE_SIZE), WatchesFor::Writes);
        }
    }

    fn on_watched_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        access: AccessKind,
    ) {
        self.1
            .lock()
            .unwrap()
            .push((process.id().0, address, access, true));
        process.watch_memory(page_of(address), WatchesFor::None);
    }
}

/// Asks for the written pages when a child starts and again when it ends.
struct Changes(Arc<Mutex<Vec<std::ops::Range<u64>>>>);

impl Hooks for Changes {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            process.take_dirty_memory();
        }
    }

    fn on_process_exiting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            *self.0.lock().unwrap() = process.take_dirty_memory();
        }
    }
}

#[test]
fn the_pages_changed_are_exactly_those_a_program_wrote_or_mapped_anew() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let changed = Arc::new(Mutex::new(Vec::new()));
        let run = run_embedder("pages", Box::new(Changes(changed.clone())), seed);
        assert_eq!(run.outcome.exit_code, 0, "{:?}", run.outcome);
        let in_buffer: Vec<u64> = changed
            .lock()
            .unwrap()
            .iter()
            .flat_map(|range| (range.start..range.end).step_by(PAGE_SIZE as usize))
            .filter(|page| buffer().contains(page))
            .map(|page| (page - BUFFER) / PAGE_SIZE)
            .collect();
        assert_eq!(in_buffer, [0, 2, 3, 4, 5, 7], "seed {seed}");
    }
}

/// A span as wide as a 48-bit address space, far wider than what the program maps.
const VAST: u64 = 1 << 48;
/// Far more than watching every mapped page of a small program takes, and far less than visiting
/// every page of `VAST` would.
const VAST_WATCH_LIMIT: Duration = Duration::from_secs(1);

/// Watches all of `VAST` for writes, then ends the watches, as each program starts; notes how
/// long that took.
struct VastWatch(Arc<Mutex<Vec<Duration>>>);

impl Hooks for VastWatch {
    fn on_program_starting(&mut self, mut process: Process<'_>) {
        let started = std::time::Instant::now();
        process.watch_memory(0..VAST, WatchesFor::Writes);
        process.watch_memory(0..VAST, WatchesFor::None);
        self.0.lock().unwrap().push(started.elapsed());
    }
}

#[test]
fn watching_a_vast_range_costs_its_mapped_pages_not_its_length() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder_below(&["pages"], None, seed, None);
        let took = Arc::new(Mutex::new(Vec::new()));
        let run = run_embedder("pages", Box::new(VastWatch(took.clone())), seed);
        assert_eq!(
            run.outcome.exit_code, plain.outcome.exit_code,
            "seed {seed}"
        );
        assert_eq!(stdout(&run), stdout(&plain), "seed {seed}");
        let took = took.lock().unwrap();
        assert!(!took.is_empty(), "seed {seed}");
        assert!(
            took.iter().all(|spent| *spent < VAST_WATCH_LIMIT),
            "seed {seed}: {took:?}"
        );
    }
}

/// Lazy memory from the two primitives: a new program's writable pages are saved, cleared and
/// watched, and each is filled from the saved copy when first touched, inside the hook or,
/// `later`, between steps through `Kernel::process`; a fork's child fills from its parent's copy.
/// A page found changed when it is filled was written past its watch.
#[derive(Default)]
struct Lazy {
    later: bool,
    saved: BTreeMap<u32, Arc<BTreeMap<u64, Vec<u8>>>>,
    pending: BTreeMap<(u32, u64), TaskId>,
    filled: Arc<Mutex<u64>>,
    written_past: Arc<Mutex<Vec<u64>>>,
    /// The process and the thread each hook named.
    named: Arc<Mutex<Vec<(u32, u32)>>>,
}

impl Lazy {
    fn fill(&self, process: &mut Process<'_>, page: std::ops::Range<u64>) {
        let saved = &self.saved[&process.id().0];
        let bytes = &saved[&page.start];
        let now = process.read_memory(page.clone()).expect("a mapped page");
        if now.iter().any(|byte| *byte != 0) {
            self.written_past.lock().unwrap().push(page.start);
        }
        process
            .write_memory(page.start, bytes)
            .expect("a mapped page");
        process.watch_memory(page, WatchesFor::None);
        *self.filled.lock().unwrap() += 1;
    }
}

impl Hooks for Lazy {
    fn on_process_starting(&mut self, process: Process<'_>) {
        if let Some(parent) = process.info().parent()
            && let Some(saved) = self.saved.get(&parent.0).cloned()
        {
            self.saved.insert(process.id().0, saved);
        }
    }

    fn on_program_starting(&mut self, mut process: Process<'_>) {
        let mut saved = BTreeMap::new();
        let zeros = vec![0; PAGE_SIZE as usize];
        for range in process.take_dirty_memory() {
            for page in (range.start..range.end).step_by(PAGE_SIZE as usize) {
                let bytes = process
                    .read_memory(page..page + PAGE_SIZE)
                    .expect("a mapped page");
                process.write_memory(page, &zeros).expect("a mapped page");
                saved.insert(page, bytes);
            }
            process.watch_memory(range, WatchesFor::All);
        }
        self.saved.insert(process.id().0, Arc::new(saved));
    }

    fn on_watched_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        address: u64,
        _: AccessKind,
    ) {
        self.named.lock().unwrap().push((process.id().0, thread.0));
        let page = page_of(address);
        if self.later {
            self.pending
                .insert((process.id().0, page.start), process.id());
        } else {
            self.fill(&mut process, page);
        }
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let pending = std::mem::take(&mut self.pending);
        for ((_, page), id) in &pending {
            let mut process = kernel.process(cpu, *id).expect("the process waits");
            self.fill(&mut process, page_of(*page));
        }
        !pending.is_empty()
    }
}

/// The scripts of tests/shell.rs that compare with the reference.
const SCRIPTS: &[&str] = &[
    "cat /etc/passwd | grep root | cut -d: -f1",
    "seq 1 2000 | sort -rn | head -n 3; echo status $?",
    "seq 1 100000 | wc -l",
    "yes | head -n 5",
    "false | true; echo $?; true | false; echo $?",
    "echo a b c | tr ' ' '\\n' | sort -r | tr '\\n' ,",
    "ls /bin | wc -l > /tmp/count; cat /tmp/count",
    "(echo sub; exit 3); echo after $?",
    "x=$(echo inner | tr a-z A-Z); echo $x",
    "sleep 0.2 & echo started; wait; echo waited $?",
    "sh -c 'exit 7' & wait $!; echo status $?",
    "{ for i in 1 2 3; do (echo job $i) & done; wait; } | sort; echo all",
    "trap 'echo got usr1' USR1; kill -USR1 $$; echo after trap",
    "sleep 5 & kill $!; wait $!; echo killed $?",
    "exec 3>/tmp/fd3; echo to3 >&3; exec 3>&-; cat /tmp/fd3",
    "ulimit -n; umask; cd /tmp && pwd",
    "echo $$ $PPID | wc -w; read x < /etc/hostname; echo $x",
];

#[test]
fn lazy_memory_runs_the_shell_scripts_as_the_reference_does() {
    for script in SCRIPTS {
        let command = ["sh", "-c", script];
        let (theirs, _) = reference::run_command(&command);
        for seed in wcontainers_kernel_test::schedule_seeds() {
            let lazy = Lazy::default();
            let (filled, written_past) = (lazy.filled.clone(), lazy.written_past.clone());
            let ours = busybox::run_ours_hooked(&command, seed, Box::new(lazy));
            let label = format!("{script:?}, seed {seed}");
            assert_eq!(
                String::from_utf8_lossy(&ours.outcome.stdout),
                String::from_utf8_lossy(&theirs.stdout),
                "{label}: standard output"
            );
            assert_eq!(
                String::from_utf8_lossy(&ours.outcome.stderr),
                String::from_utf8_lossy(&theirs.stderr),
                "{label}: standard error"
            );
            assert_eq!(
                ours.outcome.exit_code, theirs.exit_code,
                "{label}: exit code"
            );
            assert!(*filled.lock().unwrap() > 0, "{label}: nothing was filled");
            let undescribed: Vec<_> = ours
                .counters
                .iter()
                .filter(|counter| counter.reason == reason::COPY_UNDESCRIBED)
                .collect();
            assert!(undescribed.is_empty(), "{label}: {undescribed:?}");
            assert!(
                written_past.lock().unwrap().is_empty(),
                "{label}: written past the watch: {:x?}",
                written_past.lock().unwrap()
            );
        }
    }
}

/// Runs embedder.c's `mode` under lazy memory as the reference runs it, its pages filled inside
/// the hook and between steps: the same output, nothing written past a watch, no copy outside a
/// call's description or short of watched memory. Gives what each lazy run's hooks named.
fn lazy_as_the_reference(mode: &[&str]) -> Vec<Vec<(u32, u32)>> {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let mut arguments = vec![mode[0], address.as_str(), pages.as_str()];
    arguments.extend(&mode[1..]);
    let theirs = reference::run(&program, &arguments);
    let theirs = String::from_utf8_lossy(&theirs.stdout);
    let mut named = Vec::new();
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder_below(mode, None, seed, None);
        assert_eq!(stdout(&plain), theirs, "{}, seed {seed}", mode[0]);
        for later in [false, true] {
            let lazy = Lazy {
                later,
                ..Lazy::default()
            };
            let (filled, written_past) = (lazy.filled.clone(), lazy.written_past.clone());
            let hooks_named = lazy.named.clone();
            let run = run_embedder_below(mode, Some(Box::new(lazy)), seed, None);
            let label = format!("{}, seed {seed}, later {later}", mode[0]);
            assert_eq!(stdout(&run), theirs, "{label}: {:?}", run.counters);
            assert!(*filled.lock().unwrap() > 0, "{label}: nothing was filled");
            assert!(
                written_past.lock().unwrap().is_empty(),
                "{label}: written past the watch: {:x?}",
                written_past.lock().unwrap()
            );
            for counter in [
                reason::COPY_UNDESCRIBED,
                reason::WATCH_MET_AT_A_COPY,
                reason::CALL_SHORT_OF_WATCHED_MEMORY,
            ] {
                assert_eq!(
                    counted(&run, counter),
                    0,
                    "{label}: {counter}: {:?}",
                    run.counters
                );
            }
            named.push(std::mem::take(&mut *hooks_named.lock().unwrap()));
        }
    }
    named
}

#[test]
fn a_programs_own_memory_reached_by_the_process_vm_calls_meets_its_watches() {
    lazy_as_the_reference(&["vm"]);
}

/// A command line long enough to fill pages the program never touches itself.
const LONG_ARGUMENT: u64 = 3 * PAGE_SIZE;

/// Reads a long command line or environment (`file`) through /proc as the reference does, in the
/// process itself and from a child, each run under lazy memory.
fn proc_strings_meet_the_watches_of_the_process_they_name(file: &str) {
    let long = "a".repeat(LONG_ARGUMENT as usize);
    for named in lazy_as_the_reference(&[file, "self", &long]) {
        assert!(
            named.iter().all(|(process, thread)| process == thread),
            "{named:?}"
        );
    }
    // The child's read meets its parent's watches, named with the child's thread.
    for named in lazy_as_the_reference(&[file, "parent", &long]) {
        assert!(
            named.iter().any(|(process, thread)| process != thread),
            "{named:?}"
        );
    }
}

#[test]
fn a_command_line_read_through_proc_meets_the_watches_of_the_process_it_names() {
    proc_strings_meet_the_watches_of_the_process_they_name("cmdline");
}

#[test]
fn an_environment_read_through_proc_meets_the_watches_of_the_process_it_names() {
    proc_strings_meet_the_watches_of_the_process_they_name("environ");
}

/// How many of the buffer's pages embedder.c's `remap` moves, and where to.
const MOVED: u64 = PAGES / 2;
const MOVED_TO: u64 = BUFFER + 2 * PAGES * PAGE_SIZE;

/// In a child: keeps a record of written pages from its start, and watches the buffer's first
/// pages, all of the pages to be moved but the last (lazily: saved, cleared and watched All), the
/// last moved page for writes, and the page after them lazily.
#[derive(Default)]
struct Moves {
    saved: BTreeMap<u64, Vec<u8>>,
    touches: Arc<Mutex<Vec<(u64, AccessKind)>>>,
    changed: Arc<Mutex<Vec<std::ops::Range<u64>>>>,
}

impl Hooks for Moves {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_none() {
            return;
        }
        process.take_dirty_memory();
        let zeros = vec![0; PAGE_SIZE as usize];
        for index in (0..MOVED - 1).chain([MOVED]) {
            let page = page_of(BUFFER + index * PAGE_SIZE);
            let bytes = process.read_memory(page.clone()).expect("the buffer");
            process
                .write_memory(page.start, &zeros)
                .expect("the buffer");
            process.watch_memory(page.clone(), WatchesFor::All);
            self.saved.insert(page.start, bytes);
        }
        process.watch_memory(
            page_of(BUFFER + (MOVED - 1) * PAGE_SIZE),
            WatchesFor::Writes,
        );
    }

    fn on_watched_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        access: AccessKind,
    ) {
        self.touches.lock().unwrap().push((address, access));
        let page = page_of(address);
        if let Some(bytes) = self.saved.remove(&page.start) {
            process
                .write_memory(page.start, &bytes)
                .expect("a watched page");
        }
        process.watch_memory(page, WatchesFor::None);
    }

    fn on_process_exiting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            *self.changed.lock().unwrap() = process.take_dirty_memory();
        }
    }
}

#[test]
fn watches_and_the_record_follow_pages_moved_or_dropped() {
    let (to, count) = (format!("{MOVED_TO:#x}"), MOVED.to_string());
    let arguments = ["remap", to.as_str(), count.as_str()];
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder_below(&arguments, None, seed, None);
        assert_eq!(plain.outcome.exit_code, 0, "{:?}", plain.outcome);
        assert!(
            stdout(&plain).ends_with(" dropped 0\n"),
            "{}",
            stdout(&plain)
        );
        let moves = Moves::default();
        let (touches, changed) = (moves.touches.clone(), moves.changed.clone());
        let run = run_embedder_below(&arguments, Some(Box::new(moves)), seed, None);
        assert_eq!(stdout(&run), stdout(&plain), "seed {seed}");
        // The move reads the lazy pages where they are; the page watched for writes goes along
        // and its write is met where it landed; dropping a lazy page is met as a write.
        let page = |base: u64, index: u64| base + index * PAGE_SIZE;
        let mut expected: Vec<(u64, AccessKind)> = (0..MOVED - 1)
            .map(|index| (page(BUFFER, index), AccessKind::Read))
            .collect();
        expected.push((page(MOVED_TO, MOVED - 1), AccessKind::Write));
        expected.push((page(BUFFER, MOVED), AccessKind::Write));
        assert_eq!(*touches.lock().unwrap(), expected, "seed {seed}");
        // Written since the child started: every moved page where it landed, and the dropped one.
        let watched = BUFFER..page(MOVED_TO, MOVED);
        let written: Vec<u64> = changed
            .lock()
            .unwrap()
            .iter()
            .flat_map(|range| (range.start..range.end).step_by(PAGE_SIZE as usize))
            .filter(|at| watched.contains(at))
            .collect();
        let mut expected: Vec<u64> = vec![page(BUFFER, MOVED)];
        expected.extend((0..MOVED).map(|index| page(MOVED_TO, index)));
        assert_eq!(written, expected, "seed {seed}");
    }
}

/// Tries to write into a child's read-only shared page at its start.
struct SharedWrite(u64, Arc<Mutex<Vec<Result<(), MemoryError>>>>);

impl Hooks for SharedWrite {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            let result = process.write_memory(self.0, b"y");
            self.1.lock().unwrap().push(result);
        }
    }
}

#[test]
fn a_read_only_shared_page_is_not_written_by_the_embedder() {
    let at = BUFFER + PAGES * PAGE_SIZE;
    let address = format!("{at:#x}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let results = Arc::new(Mutex::new(Vec::new()));
        let run = run_embedder_below(
            &["shared", &address],
            Some(Box::new(SharedWrite(at, results.clone()))),
            seed,
            None,
        );
        assert_eq!(stdout(&run), "file x mapped x\n", "{:?}", run.outcome);
        assert_eq!(
            *results.lock().unwrap(),
            [Err(MemoryError::SharedNotWritable)]
        );
    }
}

/// In each child: keeps a record of written pages from its start, makes the buffer's second page
/// lazy (saved, cleared and watched All) and watches its third for writes.
#[derive(Default)]
struct Attributes {
    saved: BTreeMap<u64, Vec<u8>>,
    touches: Arc<Mutex<Vec<(u64, AccessKind)>>>,
    changed: Arc<Mutex<Vec<Vec<u64>>>>,
}

impl Hooks for Attributes {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_none() {
            return;
        }
        process.take_dirty_memory();
        let lazy = page_of(BUFFER + PAGE_SIZE);
        let bytes = process.read_memory(lazy.clone()).expect("the buffer");
        process
            .write_memory(lazy.start, &vec![0; PAGE_SIZE as usize])
            .expect("the buffer");
        process.watch_memory(lazy.clone(), WatchesFor::All);
        self.saved.insert(lazy.start, bytes);
        process.watch_memory(page_of(BUFFER + 2 * PAGE_SIZE), WatchesFor::Writes);
    }

    fn on_watched_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        access: AccessKind,
    ) {
        let page = page_of(address);
        self.touches.lock().unwrap().push((page.start, access));
        if let Some(bytes) = self.saved.remove(&page.start) {
            process
                .write_memory(page.start, &bytes)
                .expect("a watched page");
        }
        process.watch_memory(page, WatchesFor::None);
    }

    fn on_process_exiting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_none() {
            return;
        }
        let written = process
            .take_dirty_memory()
            .into_iter()
            .flat_map(|range| (range.start..range.end).step_by(PAGE_SIZE as usize))
            .filter(|page| buffer().contains(page))
            .map(|page| (page - BUFFER) / PAGE_SIZE)
            .collect();
        self.changed.lock().unwrap().push(written);
    }
}

#[test]
fn watches_and_the_record_stay_through_attribute_changes() {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let theirs = reference::run(&program, &["attributes", &address, &pages]);
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder_below(&["attributes"], None, seed, None);
        assert_eq!(plain.outcome.exit_code, 0, "{:?}", plain.outcome);
        let output = stdout(&plain);
        assert_eq!(
            output,
            String::from_utf8_lossy(&theirs.stdout),
            "seed {seed}"
        );
        let mut lines = output.lines();
        let count: usize = lines
            .next()
            .and_then(|line| line.strip_prefix("attributes "))
            .and_then(|count| count.parse().ok())
            .expect("the number of changes");
        let results: Vec<&str> = lines.collect();
        assert_eq!(results.len(), count, "{output}");
        for line in &results {
            assert!(line.ends_with(" 0 read x"), "seed {seed}: {line}");
        }
        let attributes = Attributes::default();
        let (touches, changed) = (attributes.touches.clone(), attributes.changed.clone());
        let run = run_embedder_below(&["attributes"], Some(Box::new(attributes)), seed, None);
        assert_eq!(stdout(&run), output, "seed {seed}");
        // Every child meets the lazy page's watch at its read and the other at its write, after
        // the change; its record holds the page written before the change and the two after.
        let expected: Vec<(u64, AccessKind)> = (0..count)
            .flat_map(|_| {
                [
                    (BUFFER + PAGE_SIZE, AccessKind::Read),
                    (BUFFER + 2 * PAGE_SIZE, AccessKind::Write),
                ]
            })
            .collect();
        assert_eq!(*touches.lock().unwrap(), expected, "seed {seed}");
        assert_eq!(
            *changed.lock().unwrap(),
            vec![vec![0, 1, 2]; count],
            "seed {seed}"
        );
    }
}

/// A child's information when it starts and when it is about to end.
#[derive(Default)]
struct Spends(Arc<Mutex<Vec<(u64, Duration)>>>);

impl Spends {
    fn note(&self, process: &Process<'_>) {
        if process.info().parent().is_some() {
            let info = process.info();
            self.0
                .lock()
                .unwrap()
                .push((info.ceiling_distance(), info.compute_used()));
        }
    }
}

impl Hooks for Spends {
    fn on_process_starting(&mut self, process: Process<'_>) {
        self.note(&process);
    }

    fn on_process_exiting(&mut self, process: Process<'_>) {
        self.note(&process);
    }
}

/// How much embedder.c's `spend` maps.
const SPENT: u64 = 1 << 20;

#[test]
fn the_distance_to_the_ceiling_and_the_compute_used_are_the_processes_own() {
    let bytes = SPENT.to_string();
    for seed in wcontainers_kernel_test::schedule_seeds() {
        for ceiling in [None, Some(CEILING)] {
            let spends = Spends::default();
            let notes = spends.0.clone();
            let run = run_embedder_below(&["spend", &bytes], Some(Box::new(spends)), seed, ceiling);
            let label = format!("seed {seed}, ceiling {ceiling:?}");
            assert_eq!(run.outcome.exit_code, 0, "{label}: {:?}", run.outcome);
            let output = stdout(&run);
            let used: Vec<u64> = output
                .strip_prefix("used ")
                .expect("the time used")
                .split_whitespace()
                .map(|part| part.parse().expect("a number"))
                .collect();
            let printed = Duration::new(used[0], used[1] as u32);
            let notes = notes.lock().unwrap();
            let [(distance_before, used_before), (distance_after, used_after)] = notes[..] else {
                panic!("{label}: {notes:?}");
            };
            assert_eq!(distance_before - distance_after, SPENT, "{label}");
            assert_eq!(used_before, Duration::ZERO, "{label}");
            assert!(
                printed > Duration::ZERO && printed <= used_after,
                "{label}: printed {printed:?}, at the end {used_after:?}"
            );
        }
    }
}

/// Watches the buffer's writes in every process only as it exits, and meets each watch inside
/// the hook.
struct WatchAtExit;

impl Hooks for WatchAtExit {
    fn on_process_exiting(&mut self, mut process: Process<'_>) {
        process.watch_memory(buffer(), WatchesFor::Writes);
    }

    fn on_watched_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        _: AccessKind,
    ) {
        process.watch_memory(page_of(address), WatchesFor::None);
    }
}

#[test]
fn a_watch_set_as_a_process_exits_changes_nothing_the_program_sees() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder_below(&["calls"], None, seed, None);
        assert_eq!(plain.outcome.exit_code, 0, "{:?}", plain.outcome);
        // The child that shares the memory clears the parent's word as it ends.
        assert!(
            stdout(&plain).contains("child cleartid 1"),
            "{}",
            stdout(&plain)
        );
        // The kernel's stores at a thread's end come before the hook, so the watch has nothing
        // left to meet and the word is cleared as without it.
        let run = run_embedder("calls", Box::new(WatchAtExit), seed);
        assert_eq!(stdout(&run), stdout(&plain), "seed {seed}");
        for counter in [
            reason::COPY_UNDESCRIBED,
            reason::WATCH_MET_AT_A_COPY,
            reason::CALL_SHORT_OF_WATCHED_MEMORY,
        ] {
            assert_eq!(counted(&run, counter), 0, "{counter}: {:?}", run.counters);
        }
    }
}

/// In each child, watches the buffer's second page for writes; each watch met is ended and the
/// next page watched All, which a call's walk may have passed already.
#[derive(Default)]
struct Chaser {
    later: bool,
    pending: Vec<(TaskId, u64)>,
    touches: Arc<Mutex<Vec<(u64, AccessKind)>>>,
}

impl Chaser {
    fn chase(process: &mut Process<'_>, address: u64) {
        let page = page_of(address);
        process.watch_memory(page.clone(), WatchesFor::None);
        process.watch_memory(page.end..page.end + PAGE_SIZE, WatchesFor::All);
    }
}

impl Hooks for Chaser {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            process.watch_memory(page_of(BUFFER + PAGE_SIZE), WatchesFor::Writes);
        }
    }

    fn on_watched_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        access: AccessKind,
    ) {
        self.touches
            .lock()
            .unwrap()
            .push((page_of(address).start, access));
        if self.later {
            self.pending.push((process.id(), address));
        } else {
            Self::chase(&mut process, address);
        }
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let pending = std::mem::take(&mut self.pending);
        for (id, address) in &pending {
            let mut process = kernel.process(cpu, *id).expect("the process waits");
            Self::chase(&mut process, *address);
        }
        !pending.is_empty()
    }
}

#[test]
fn a_watch_raised_behind_a_calls_walk_is_met_before_the_call_acts() {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let theirs = reference::run(&program, &["scatter", &address, &pages]);
    let theirs = String::from_utf8_lossy(&theirs.stdout);
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder_below(&["scatter"], None, seed, None);
        assert_eq!(stdout(&plain), theirs, "seed {seed}");
        for later in [false, true] {
            let chaser = Chaser {
                later,
                ..Default::default()
            };
            let touches = chaser.touches.clone();
            let run = run_embedder("scatter", Box::new(chaser), seed);
            assert_eq!(stdout(&run), theirs, "seed {seed}, later {later}");
            // recvfrom's walk meets the data's page, then the length's, then the address's: the
            // address's page is watched while the walk is on the length, and the length's while
            // it is on the address, so the walk starts again for it.
            let page = |index: u64| BUFFER + index * PAGE_SIZE;
            assert_eq!(
                *touches.lock().unwrap(),
                [
                    (page(1), AccessKind::Write),
                    (page(2), AccessKind::Write),
                    (page(3), AccessKind::Write)
                ],
                "seed {seed}, later {later}"
            );
            for counter in [
                reason::COPY_UNDESCRIBED,
                reason::WATCH_MET_AT_A_COPY,
                reason::CALL_SHORT_OF_WATCHED_MEMORY,
            ] {
                assert_eq!(counted(&run, counter), 0, "{counter}: {:?}", run.counters);
            }
        }
    }
}

/// What the kernel was announced writing: the process, the thread, the range.
type Written = Arc<Mutex<Vec<(u32, u32, std::ops::Range<u64>)>>>;

/// Records what the kernel wrote in each child.
#[derive(Default)]
struct KernelWrites {
    children: BTreeSet<u32>,
    written: Written,
}

impl Hooks for KernelWrites {
    fn on_process_starting(&mut self, process: Process<'_>) {
        if process.info().parent().is_some() {
            self.children.insert(process.id().0);
        }
    }

    fn on_memory_written_by_kernel(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        range: std::ops::Range<u64>,
    ) {
        if self.children.contains(&process.id().0) {
            (self.written.lock().unwrap()).push((process.id().0, thread.0, range));
        }
    }
}

#[test]
fn the_kernels_writes_are_announced_exactly() {
    const LENGTH: u64 = 10;
    let at = BUFFER + PAGE_SIZE + 64;
    let time = BUFFER + 2 * PAGE_SIZE + size_of::<u64>() as u64;
    let word = |index: u64| {
        let start = BUFFER + 3 * PAGE_SIZE + index * size_of::<u32>() as u64;
        start..start + size_of::<u32>() as u64
    };
    let length = LENGTH.to_string();
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let hooks = KernelWrites::default();
        let written = hooks.written.clone();
        let run = run_embedder_below(
            &["kernelwrites", &length],
            Some(Box::new(hooks)),
            seed,
            None,
        );
        assert_eq!(run.outcome.exit_code, 0, "{:?}", run.outcome);
        let written = written.lock().unwrap();
        // Each was written for a thread of the process it went to, a process of its own.
        assert!(written.iter().all(|(process, thread, _)| process == thread));
        // Then the signal's frame, on the alternate stack.
        let stack = BUFFER + 4 * PAGE_SIZE..BUFFER + 6 * PAGE_SIZE;
        let fixed = written.len().min(5);
        assert!(
            written.len() > fixed
                && written[fixed..].iter().all(|(_, _, range)| {
                    stack.start <= range.start && range.start < range.end && range.end <= stack.end
                }),
            "seed {seed}: {written:?}"
        );
        let processes: Vec<u32> = written[..fixed].iter().map(|write| write.0).collect();
        let ranges: Vec<std::ops::Range<u64>> = written[..fixed]
            .iter()
            .map(|write| write.2.clone())
            .collect();
        assert_eq!(
            ranges,
            [
                at..at + LENGTH,
                time..time + size_of::<general::__kernel_timespec>() as u64,
                word(0),
                word(1),
                word(2),
            ],
            "seed {seed}: {written:?}"
        );
        // The child's own, then its vfork child's end, then the killed sharer's end; the frame is
        // the child's.
        assert!(written[fixed..].iter().all(|write| write.0 == processes[0]));
        let [child, _, _, vforked, sharer] = processes[..] else {
            panic!("seed {seed}: {written:?}");
        };
        assert_eq!(
            processes,
            [child, child, child, vforked, sharer],
            "seed {seed}"
        );
        assert!(child != vforked && child != sharer && vforked != sharer);
    }
}

/// Answers getpid itself, makes every write to standard output one byte shorter, and changes
/// getuid's answer.
struct Proxy;

/// What the proxy answers for getpid and getuid.
const PROXY_PID: u64 = 4242;
const PROXY_UID: u64 = 77;

impl Hooks for Proxy {
    fn intercept_system_call_request(
        &mut self,
        _: Process<'_>,
        _: TaskId,
        mut request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        match u32::try_from(request.number) {
            Ok(general::__NR_getpid) => return ControlFlow::Break(Ok(PROXY_PID)),
            Ok(general::__NR_write)
                if request.arguments[0] == u64::from(general::STDOUT_FILENO) =>
            {
                request.arguments[2] -= 1;
            }
            _ => {}
        }
        ControlFlow::Continue(request)
    }

    fn intercept_system_call_response(
        &mut self,
        _: Process<'_>,
        _: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        if request.number == u64::from(general::__NR_getuid) {
            assert_eq!(response, Ok(0));
            return Ok(PROXY_UID);
        }
        response
    }
}

#[test]
fn the_embedder_answers_and_changes_system_calls() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder_below(&["proxy"], None, seed, None);
        let pid = stdout(&plain);
        assert!(pid.starts_with("pid 1\nuid 0\n"), "{pid}");
        let run = run_embedder("proxy", Box::new(Proxy), seed);
        assert_eq!(
            stdout(&run),
            format!("pid {PROXY_PID}uid {PROXY_UID}"),
            "seed {seed}"
        );
    }
}

/// One call the proxy was asked about: the process, the thread, the call's number, and the
/// response, none for the request.
type Asked = (u32, u32, u64, Option<SystemCallResponse>);

/// Notes every request and response the proxy is asked about, changing nothing.
struct CallLog(Arc<Mutex<Vec<Asked>>>);

impl Hooks for CallLog {
    fn intercept_system_call_request(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        let entry = (process.id().0, thread.0, request.number, None);
        self.0.lock().unwrap().push(entry);
        ControlFlow::Continue(request)
    }

    fn intercept_system_call_response(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        let entry = (process.id().0, thread.0, request.number, Some(response));
        self.0.lock().unwrap().push(entry);
        response
    }
}

/// How many requests and responses `asked` holds for the call `number`.
fn asked_about(asked: &[Asked], number: u32) -> (usize, usize) {
    let of = asked
        .iter()
        .filter(|(_, _, asked, _)| *asked == u64::from(number));
    let responses = of.clone().filter(|(.., response)| response.is_some());
    (of.count() - responses.clone().count(), responses.count())
}

#[test]
fn a_new_program_and_a_handlers_return_are_answered_as_at_linuxs_exit_stop() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let run = run_embedder("paths", Box::new(CallLog(asked.clone())), seed);
        assert!(
            stdout(&run).contains("threaded exec 0\n"),
            "{:?}",
            run.outcome
        );
        let asked = std::mem::take(&mut *asked.lock().unwrap());
        // Each exec answers once, a successful one with 0 in the new program, to the thread
        // that carries the process's number by then.
        let (requests, responses) = asked_about(&asked, general::__NR_execve);
        assert_eq!(requests, responses, "{asked:?}");
        let started: Vec<_> = asked
            .iter()
            .filter(|entry| entry.2 == u64::from(general::__NR_execve) && entry.3 == Some(Ok(0)))
            .collect();
        assert!(started.len() >= 2, "{started:?}");
        assert!(
            started
                .iter()
                .all(|(process, thread, ..)| process == thread)
        );
        let asked = Arc::new(Mutex::new(Vec::new()));
        let length = PAGE_SIZE.to_string();
        let run = run_embedder_below(
            &["kernelwrites", &length],
            Some(Box::new(CallLog(asked.clone()))),
            seed,
            None,
        );
        assert_eq!(run.outcome.exit_code, 0, "{:?}", run.outcome);
        let asked = asked.lock().unwrap();
        let (requests, responses) = asked_about(&asked, general::__NR_rt_sigreturn);
        assert!(requests > 0 && requests == responses, "{asked:?}");
    }
}

/// What `on_signal_delivering` was given: the thread, and the record's number, code and value.
type Delivered = Arc<Mutex<Vec<(TaskId, i32, i32, i32)>>>;

/// A record's number, code and value (`si_value.sival_int`, the bytes a handler reads as it).
fn signal_fields(signal: &SignalInformation) -> (i32, i32, i32) {
    // SAFETY: the kernel builds every record it hands over from all of its bytes.
    let header = unsafe { signal.__bindgen_anon_1.__bindgen_anon_1 };
    // SAFETY: as above.
    let value = unsafe { header._sifields._rt._sigval.sival_int };
    (header.si_signo, header.si_code, value)
}

/// A record of `signal` queued with `value` from outside the container (no sender process).
fn queued_signal(signal: u32, value: i32) -> SignalInformation {
    // SAFETY: every member of the record is made of integers, for which zero is valid.
    let mut record: SignalInformation = unsafe { std::mem::zeroed() };
    // SAFETY: a record built from zero has every member's bytes set.
    let header = unsafe { &mut record.__bindgen_anon_1.__bindgen_anon_1 };
    header.si_signo = signal as i32;
    header.si_code = general::SI_QUEUE;
    header._sifields._rt._sigval.sival_int = value;
    record
}

/// Lines `signal S code C value V` of embedder.c's output, as the hook's fields.
fn handled_signals(output: &str) -> Vec<(i32, i32, i32)> {
    output
        .lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.split(' ').collect();
            match words.as_slice() {
                ["signal", signal, "code", code, "value", value] => Some((
                    signal.parse().unwrap(),
                    code.parse().unwrap(),
                    value.parse().unwrap(),
                )),
                _ => None,
            }
        })
        .collect()
}

/// Records every signal the kernel announces, after running every operation of the handle.
#[derive(Default)]
struct SignalRecorder {
    delivered: Delivered,
}

impl Hooks for SignalRecorder {
    fn on_signal_delivering(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        signal: &SignalInformation,
    ) {
        assert!(process.info().memory_mapped() > 0);
        let _ = process.take_dirty_memory();
        let (number, code, value) = signal_fields(signal);
        self.delivered
            .lock()
            .unwrap()
            .push((thread, number, code, value));
    }
}

#[test]
fn every_signal_a_thread_takes_to_act_on_is_announced_first_with_its_record() {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let theirs = reference::run(&program, &["signals", &address, &pages]);
    let theirs = String::from_utf8_lossy(&theirs.stdout).into_owned();
    let handled = handled_signals(&theirs);
    // Queued, let through, sent by another thread during a read, and a fault; not the one
    // sigwaitinfo took nor the ignored one.
    assert_eq!(handled.len(), 4, "{theirs}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let recorder = SignalRecorder::default();
        let delivered = recorder.delivered.clone();
        let run = run_embedder("signals", Box::new(recorder), seed);
        assert_eq!(stdout(&run), theirs, "seed {seed}");
        let delivered = delivered.lock().unwrap();
        let fields: Vec<(i32, i32, i32)> = delivered
            .iter()
            .map(|&(_, number, code, value)| (number, code, value))
            .collect();
        assert_eq!(fields, handled, "seed {seed}");
        // The one sent by another thread is announced in the thread it was sent to.
        let first = delivered[0].0;
        assert!(delivered.iter().all(|(thread, ..)| *thread == first));
    }
}

/// When the embedder sends embedder.c's `interrupted` child its signal.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Sending {
    /// Inside the proxy's request hook of the read, before the read runs.
    AtRequest,
    /// Inside the watch hook of the read, which then waits on the watch until between steps.
    DuringWatchWait,
    /// Between steps, while the read waits for data.
    BetweenSteps,
}

struct SignalSender {
    sending: Sending,
    delivered: Delivered,
    reader: Option<(TaskId, TaskId)>,
    sent: bool,
    watch_lifted: bool,
}

impl SignalSender {
    /// Sends the reader SIGUSR1 with 42, and two that are not sent: one to a thread of another
    /// process and one with no number.
    fn send(&mut self, process: &mut Process<'_>, thread: TaskId) {
        process.send_signal(thread, queued_signal(general::SIGUSR1, SENT_VALUE));
        let parent = process.info().parent().expect("a parent");
        process.send_signal(parent, queued_signal(general::SIGUSR1, SENT_VALUE));
        process.send_signal(thread, queued_signal(0, SENT_VALUE));
        self.sent = true;
    }
}

/// The value embedder.c's parent queues with its signal.
const SENT_VALUE: i32 = 42;

impl Hooks for SignalSender {
    fn intercept_system_call_request(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        let read =
            request.number == u64::from(general::__NR_read) && request.arguments[1] == BUFFER;
        if read && self.reader.is_none() {
            self.reader = Some((process.id(), thread));
            match self.sending {
                Sending::AtRequest => self.send(&mut process, thread),
                Sending::DuringWatchWait => process.watch_memory(page_of(BUFFER), WatchesFor::All),
                Sending::BetweenSteps => {}
            }
        }
        ControlFlow::Continue(request)
    }

    fn on_watched_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        _: u64,
        _: AccessKind,
    ) {
        if !self.sent {
            self.send(&mut process, thread);
        }
    }

    fn on_signal_delivering(&mut self, _: Process<'_>, thread: TaskId, signal: &SignalInformation) {
        let (number, code, value) = signal_fields(signal);
        self.delivered
            .lock()
            .unwrap()
            .push((thread, number, code, value));
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let Some((id, thread)) = self.reader else {
            return false;
        };
        let Some(mut process) = kernel.process(cpu, id) else {
            return false;
        };
        match self.sending {
            Sending::BetweenSteps if !self.sent => {
                self.send(&mut process, thread);
                true
            }
            Sending::DuringWatchWait if self.sent && !self.watch_lifted => {
                process.watch_memory(page_of(BUFFER), WatchesFor::None);
                self.watch_lifted = true;
                true
            }
            _ => false,
        }
    }
}

#[test]
fn a_signal_the_embedder_sends_interrupts_a_read_as_one_another_process_sends() {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let theirs = reference::run(&program, &["interrupted", &address, &pages, "parent"]);
    let theirs = String::from_utf8_lossy(&theirs.stdout).into_owned();
    let sent = (general::SIGUSR1 as i32, general::SI_QUEUE, SENT_VALUE);
    assert_eq!(handled_signals(&theirs), [sent], "{theirs}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let ours = run_embedder_below(&["interrupted", "parent"], None, seed, None);
        assert_eq!(stdout(&ours), theirs, "seed {seed}");
        for sending in [
            Sending::AtRequest,
            Sending::DuringWatchWait,
            Sending::BetweenSteps,
        ] {
            let sender = SignalSender {
                sending,
                delivered: Delivered::default(),
                reader: None,
                sent: false,
                watch_lifted: false,
            };
            let delivered = sender.delivered.clone();
            let run =
                run_embedder_below(&["interrupted", "none"], Some(Box::new(sender)), seed, None);
            let label = format!("{sending:?}, seed {seed}");
            assert_eq!(stdout(&run), theirs, "{label}: {:?}", run.counters);
            let delivered = delivered.lock().unwrap();
            assert_eq!(delivered.len(), 1, "{label}: {delivered:?}");
            assert_eq!(
                (delivered[0].1, delivered[0].2, delivered[0].3),
                sent,
                "{label}"
            );
            // The one to another process's thread and the one with no number.
            assert_eq!(
                counted(&run, reason::SIGNAL_NOT_SENT),
                2,
                "{label}: {:?}",
                run.counters
            );
        }
    }
}

/// The shared pages announced, by process and page: the source and where in it the page is.
type SharedPages = BTreeMap<(TaskId, u64), (MemoryObject, u64)>;

/// Keeps the shared memory each process is announced to map, page by page, checking that every
/// end names what was announced; at each `sched_yield` keeps a copy, with the process yielding.
#[derive(Default)]
struct SharingRecorder {
    pages: Arc<Mutex<SharedPages>>,
    steps: Arc<Mutex<Vec<(TaskId, SharedPages)>>>,
    /// Processes announced to map anything, and to end mapping anything.
    mapped_in: Arc<Mutex<BTreeSet<TaskId>>>,
    unmapped_in: Arc<Mutex<BTreeSet<TaskId>>>,
}

impl Hooks for SharingRecorder {
    fn intercept_system_call_request(
        &mut self,
        process: Process<'_>,
        _: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        if request.number == u64::from(general::__NR_sched_yield) {
            let pages = self.pages.lock().unwrap().clone();
            self.steps.lock().unwrap().push((process.id(), pages));
        }
        ControlFlow::Continue(request)
    }

    fn on_shared_memory_mapping(
        &mut self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: std::ops::Range<u64>,
    ) {
        assert!(process.info().memory_mapped() > 0);
        self.mapped_in.lock().unwrap().insert(process.id());
        let mut pages = self.pages.lock().unwrap();
        for page in range.clone().step_by(PAGE_SIZE as usize) {
            let place = (source, offset + (page - range.start));
            let before = pages.insert((process.id(), page), place);
            assert_eq!(before, None, "{:?} {page:#x} announced twice", process.id());
        }
    }

    fn on_shared_memory_unmapped(
        &mut self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: std::ops::Range<u64>,
    ) {
        self.unmapped_in.lock().unwrap().insert(process.id());
        let mut pages = self.pages.lock().unwrap();
        for page in range.clone().step_by(PAGE_SIZE as usize) {
            let place = (source, offset + (page - range.start));
            let before = pages.remove(&(process.id(), page));
            assert_eq!(before, Some(place), "{:?} {page:#x} ended", process.id());
        }
    }
}

/// embedder.c's `sharing` steps: each name, and the shared pages /proc/self/maps showed then
/// (page, offset in the source).
fn sharing_steps(output: &str) -> Vec<(String, BTreeMap<u64, u64>)> {
    let mut steps: Vec<(String, BTreeMap<u64, u64>)> = Vec::new();
    for line in output.lines() {
        if let Some(name) = line.strip_prefix("step ") {
            steps.push((name.to_owned(), BTreeMap::new()));
            continue;
        }
        let words: Vec<&str> = line.split(' ').collect();
        let [range, _, offset] = words.as_slice() else {
            continue;
        };
        let Some((start, end)) = range.split_once('-') else {
            continue;
        };
        let hex = |text: &str| u64::from_str_radix(text, 16).unwrap();
        let (start, end, offset) = (hex(start), hex(end), hex(offset));
        let (_, pages) = steps.last_mut().expect("a step");
        for page in (start..end).step_by(PAGE_SIZE as usize) {
            pages.insert(page, offset + (page - start));
        }
    }
    steps
}

#[test]
fn shared_memory_is_announced_per_process_as_it_starts_and_ends_as_proc_maps_shows_it() {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let theirs = reference::run(&program, &["sharing", &address, &pages]);
    let theirs = String::from_utf8_lossy(&theirs.stdout).into_owned();
    let steps = sharing_steps(&theirs);
    let names: Vec<&str> = steps.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "readonly",
            "writable",
            "anonymous",
            "segment",
            "memfd",
            "moved",
            "replaced",
            "child"
        ],
        "{theirs}"
    );
    let segment: i32 = theirs
        .lines()
        .find_map(|line| line.strip_prefix("segment "))
        .expect("a segment")
        .parse()
        .unwrap();
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let recorder = SharingRecorder::default();
        let (announced, taken) = (recorder.pages.clone(), recorder.steps.clone());
        let (mapped_in, unmapped_in) = (recorder.mapped_in.clone(), recorder.unmapped_in.clone());
        let run = run_embedder("sharing", Box::new(recorder), seed);
        assert_eq!(stdout(&run), theirs, "seed {seed}");
        let taken = taken.lock().unwrap();
        assert_eq!(taken.len(), steps.len(), "seed {seed}");
        let keeper = taken.last().expect("the child's step").0;
        let mut previous = BTreeSet::new();
        for ((name, shown), (process, all)) in steps.iter().zip(taken.iter()) {
            let mine: BTreeMap<u64, u64> = all
                .iter()
                .filter(|((owner, _), _)| owner == process)
                .map(|((_, page), (_, offset))| (*page, *offset))
                .collect();
            // Nobody could write the file through a mapping yet.
            let expected = if name == "readonly" {
                BTreeMap::new()
            } else {
                shown.clone()
            };
            assert_eq!(mine, expected, "seed {seed}, step {name}");
            let sources: BTreeSet<MemoryObject> = all
                .iter()
                .filter(|((owner, _), _)| owner == process)
                .map(|(_, (source, _))| *source)
                .collect();
            // What each step adds to the sources of the step before it.
            let added: Vec<MemoryObject> = sources.difference(&previous).copied().collect();
            let removed: Vec<MemoryObject> = previous.difference(&sources).copied().collect();
            let label = format!("seed {seed}, step {name}: added {added:?}, removed {removed:?}");
            match name.as_str() {
                "readonly" | "moved" | "replaced" => {
                    assert!(added.is_empty() && removed.is_empty(), "{label}")
                }
                "writable" => assert!(
                    matches!(added[..], [MemoryObject::File { .. }]) && removed.is_empty(),
                    "{label}"
                ),
                "anonymous" => assert!(
                    matches!(added[..], [MemoryObject::Anonymous(_)]) && removed.is_empty(),
                    "{label}"
                ),
                "segment" => assert!(
                    added == [MemoryObject::SystemVSegment(SystemVSegmentId(segment))]
                        && removed.is_empty(),
                    "{label}"
                ),
                "memfd" => assert!(
                    matches!(added[..], [MemoryObject::File { .. }])
                        && removed == [MemoryObject::SystemVSegment(SystemVSegmentId(segment))],
                    "{label}"
                ),
                _ => assert!(added.is_empty() && sources.len() == 1, "{label}"),
            }
            previous = sources;
            // The child that kept the read-only mapping is announced when the file becomes
            // writable through the parent's.
            if name == "writable" {
                let kept = &steps.last().expect("the child's step").1;
                let its: BTreeMap<u64, u64> = all
                    .iter()
                    .filter(|((owner, _), _)| *owner == keeper)
                    .map(|((_, page), (_, offset))| (*page, *offset))
                    .collect();
                assert_eq!(&its, kept, "seed {seed}: the child at the parent's step");
            }
        }
        // Every start ended, in the parent, the child that kept a mapping and the one that ran
        // a program.
        assert!(announced.lock().unwrap().is_empty(), "seed {seed}");
        let (mapped_in, unmapped_in) = (mapped_in.lock().unwrap(), unmapped_in.lock().unwrap());
        assert_eq!(mapped_in.len(), 3, "seed {seed}: {mapped_in:?}");
        assert_eq!(*mapped_in, *unmapped_in, "seed {seed}");
    }
}

#[test]
fn every_operation_of_the_handle_is_legal_in_the_shared_memory_hooks() {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let theirs = reference::run(&program, &["sharing", &address, &pages]);
    let theirs = String::from_utf8_lossy(&theirs.stdout).into_owned();
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let run = run_embedder("sharing", Box::new(Recorder::default()), seed);
        assert_eq!(stdout(&run), theirs, "seed {seed}: {:?}", run.counters);
    }
}

/// The hook, the process and its memory id.
type MemoryIdSeen = (&'static str, TaskId, wcontainers_abi::MemoryId);

/// Keeps the memory id each process is announced with at each of its hooks.
#[derive(Default)]
struct MemoryIds {
    seen: Arc<Mutex<Vec<MemoryIdSeen>>>,
}

impl Hooks for MemoryIds {
    fn on_process_starting(&mut self, process: Process<'_>) {
        let id = process.memory_id();
        self.seen
            .lock()
            .unwrap()
            .push(("process", process.id(), id));
    }
    fn on_program_starting(&mut self, process: Process<'_>) {
        let id = process.memory_id();
        self.seen
            .lock()
            .unwrap()
            .push(("program", process.id(), id));
    }
}

#[test]
fn processes_in_one_memory_have_one_memory_id_and_a_new_program_a_new_one() {
    // The paths mode makes forked children, one that runs a program from a second thread, and a
    // vfork child that runs a program.
    let ids = MemoryIds::default();
    let seen = ids.seen.clone();
    run_embedder(
        "paths",
        Box::new(ids),
        wcontainers_kernel_test::schedule_seeds().next().unwrap(),
    );
    let seen = seen.lock().unwrap();
    let first = seen[0].2;
    let (mut shared, mut fresh) = (0, 0);
    for (index, (hook, process, memory)) in seen.iter().enumerate() {
        let earlier: Vec<_> = seen[..index].iter().map(|(_, _, memory)| *memory).collect();
        match *hook {
            // A new program always runs in memory no process had before.
            "program" if index > 1 => {
                assert!(!earlier.contains(memory), "{process:?}: {seen:?}");
                fresh += 1;
            }
            "process" if earlier.contains(memory) => shared += 1,
            _ => {}
        }
    }
    assert_eq!(seen[1].2, first);
    assert!(shared > 0 && fresh > 0, "{seen:?}");
}

thread_local! {
    /// Tasks a hook suspended, for the test to put back between steps (they are the kernel's
    /// records, which stay on the thread that drives it).
    static SUSPENDED: std::cell::RefCell<Vec<SuspendedTask>> = const { std::cell::RefCell::new(Vec::new()) };
    /// A process context taken between steps, kept as [`SUSPENDED`] is.
    static CONTEXT: std::cell::RefCell<Option<wcontainers_kernel::ProcessContext>> = const { std::cell::RefCell::new(None) };
}

/// How the suspending test takes tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SuspendPlan {
    /// From hooks: the other threads, waiting in their calls, at the main thread's second sleep, kept away while the main thread
    /// wakes the futex and lets the mutex go; the main thread itself at its write, before the
    /// call, and again once the call has begun (refused).
    Hooks,
    /// Between steps, while every thread waits: one at a time, every thread, the whole process,
    /// and the process through a second kernel.
    Kernel,
}

/// Where the suspending test is in its plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SuspendStage {
    /// Waiting for the main thread's sleep.
    Start,
    /// The main thread slept once; the others wait in their calls.
    SleptOnce,
    /// The other threads are away.
    Away,
    /// The main thread is away too, taken at its write.
    MainAway,
    /// The main thread is back.
    MainBack,
    /// The main thread waits to join the others, still away.
    Joining,
    /// Every thread waits in its call.
    Waiting,
    Done,
}

/// Suspends and resumes embedder.c's `suspend` threads as `plan` says, putting them back in
/// `order`.
struct Suspender {
    plan: SuspendPlan,
    order: [usize; SUSPENDED_THREADS],
    stage: SuspendStage,
    process: Option<TaskId>,
    threads: Vec<TaskId>,
    /// What went otherwise than the test expects.
    wrong: Arc<Mutex<Vec<String>>>,
    /// How many tasks a hook got asking for its own thread once its call had begun.
    refused_in_step: Arc<Mutex<Option<usize>>>,
    /// Every step of the plan ran.
    done: Arc<std::sync::atomic::AtomicBool>,
}

/// The threads embedder.c's `suspend` starts besides its main thread.
const SUSPENDED_THREADS: usize = 4;

impl Suspender {
    fn check(&self, what: &str, holds: bool) {
        if !holds {
            self.wrong.lock().unwrap().push(what.to_owned());
        }
    }

    /// Every thread, the main one first.
    fn every_thread(&self) -> Vec<TaskId> {
        self.process
            .into_iter()
            .chain(self.threads.clone())
            .collect()
    }

    /// Takes `tasks` and puts them back in this test's order, checking each step.
    fn one_by_one(&self, kernel: &mut Kernel, cpu: &mut dyn Cpu) {
        let mut taken = Vec::new();
        for &at in &self.order {
            let thread = self.threads[at];
            let got: Vec<_> = kernel.suspend_tasks(cpu, &[thread]).collect();
            self.check("one thread taken", got.len() == 1 && got[0].0 == thread);
            taken.extend(got.into_iter().map(|(_, task)| task));
        }
        let absent = TaskId(u32::MAX);
        self.check(
            "a task not on the kernel is absent",
            kernel.suspend_tasks(cpu, &[absent]).next().is_none(),
        );
        for task in taken {
            self.check(
                "a thread put back",
                kernel.resume_tasks(cpu, [task]).next().is_none(),
            );
        }
    }

    /// Takes the whole process and puts it back, its tasks first without the context.
    fn whole_process(&self, kernel: &mut Kernel, cpu: &mut dyn Cpu) {
        let process = self.process.expect("the process");
        let config = ExecutionConfig::default();
        let copy = kernel.copy_process_context(process).expect("a copy");
        let every = self.every_thread();
        let got: Vec<_> = kernel.suspend_tasks(cpu, &every).collect();
        self.check("every thread taken", got.len() == every.len());
        let tasks = got.into_iter().map(|(_, task)| task).collect::<Vec<_>>();
        self.check(
            "every thread put back",
            kernel.resume_tasks(cpu, tasks).next().is_none(),
        );
        let (context, tasks) = kernel.suspend_process(cpu, process).expect("the process");
        let tasks: Vec<_> = tasks.map(|(_, task)| task).collect();
        self.check("the process's every task", tasks.len() == every.len());
        self.check(
            "the process is gone",
            kernel.process(cpu, process).is_none()
                && kernel.copy_process_context(process).is_none(),
        );
        let back: Vec<_> = kernel.resume_tasks(cpu, tasks).collect();
        self.check(
            "tasks without their context handed back",
            back.len() == every.len(),
        );
        self.check(
            "the context put back",
            kernel.resume_process(context, config).is_none(),
        );
        let twice = kernel.resume_process(copy, config);
        self.check(
            "a context already there handed back",
            twice.as_ref().map(|context| context.id()) == Some(process),
        );
        self.check(
            "the tasks put back with their context",
            kernel.resume_tasks(cpu, back).next().is_none(),
        );
    }

    /// Moves the process to a second kernel, checks it is there as it was, and brings it back.
    fn second_kernel(&self, kernel: &mut Kernel, cpu: &mut dyn Cpu) {
        let process = self.process.expect("the process");
        let config = ExecutionConfig::default();
        let memory = kernel.process(cpu, process).expect("here").memory_id();
        let mapped = kernel
            .process(cpu, process)
            .expect("here")
            .info()
            .memory_mapped();
        let (context, tasks) = kernel.suspend_process(cpu, process).expect("the process");
        let tasks: Vec<_> = tasks.map(|(_, task)| task).collect();
        let mut other = Kernel::new();
        let tasks: Vec<_> = other.resume_tasks(cpu, tasks).collect();
        self.check(
            "no context on the second kernel yet",
            tasks.len() == self.every_thread().len(),
        );
        self.check(
            "the context on the second kernel",
            other.resume_process(context, config).is_none(),
        );
        self.check(
            "the tasks on the second kernel",
            other.resume_tasks(cpu, tasks).next().is_none(),
        );
        let there = other.process(cpu, process).expect("on the second kernel");
        self.check(
            "the same memory on the second kernel",
            there.memory_id() == memory && there.info().memory_mapped() == mapped,
        );
        let (context, tasks) = other.suspend_process(cpu, process).expect("the process");
        let tasks: Vec<_> = tasks.map(|(_, task)| task).collect();
        self.check(
            "every task from the second kernel",
            tasks.len() == self.every_thread().len(),
        );
        self.check(
            "the context back on the first kernel",
            kernel.resume_process(context, config).is_none(),
        );
        let ordered: Vec<_> = self
            .order
            .iter()
            .map(|&at| self.threads[at])
            .chain(self.process)
            .collect();
        let mut tasks = tasks;
        tasks.sort_by_key(|task| ordered.iter().position(|id| *id == task.id()));
        self.check(
            "the tasks back on the first kernel",
            kernel.resume_tasks(cpu, tasks).next().is_none(),
        );
    }
}

impl Suspender {
    /// The other threads, in this test's order.
    fn others(&self) -> Vec<TaskId> {
        self.order.iter().map(|&at| self.threads[at]).collect()
    }

    /// Puts back what the hooks took of the main thread (`main`) or of the others.
    fn put_back(&self, kernel: &mut Kernel, cpu: &mut dyn Cpu, main: bool) {
        let tasks: Vec<SuspendedTask> = SUSPENDED.with_borrow_mut(|stash| {
            let (chosen, kept) = std::mem::take(stash)
                .into_iter()
                .partition(|task| (Some(task.id()) == self.process) == main);
            *stash = kept;
            chosen
        });
        self.check("the tasks a hook took put back", !tasks.is_empty());
        self.check(
            "the tasks a hook took put back",
            kernel.resume_tasks(cpu, tasks).next().is_none(),
        );
    }

    fn take_in_hook(&self, process: &mut Process<'_>, tasks: &[TaskId]) {
        let taken: Vec<_> = process.suspend_tasks(tasks).collect();
        self.check("a hook takes what it names", taken.len() == tasks.len());
        SUSPENDED.with_borrow_mut(|stash| stash.extend(taken.into_iter().map(|(_, task)| task)));
    }
}

impl Hooks for Suspender {
    fn on_process_starting(&mut self, process: Process<'_>) {
        self.process.get_or_insert(process.id());
    }

    fn on_thread_starting(&mut self, _: Process<'_>, thread: TaskId) {
        self.threads.push(thread);
    }

    fn intercept_system_call_request(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        if Some(thread) != self.process {
            return ControlFlow::Continue(request);
        }
        let number = request.number;
        let sleep = number == u64::from(general::__NR_nanosleep);
        match (self.plan, self.stage) {
            (SuspendPlan::Hooks, SuspendStage::Start) if sleep => {
                self.stage = SuspendStage::SleptOnce;
            }
            (SuspendPlan::Hooks, SuspendStage::SleptOnce) if sleep => {
                self.take_in_hook(&mut process, &self.others());
                self.stage = SuspendStage::Away;
            }
            (SuspendPlan::Hooks, SuspendStage::Away)
                if number == u64::from(general::__NR_write) =>
            {
                self.take_in_hook(&mut process, &[thread]);
                self.stage = SuspendStage::MainAway;
            }
            (SuspendPlan::Hooks, SuspendStage::MainBack)
                if number == u64::from(general::__NR_futex)
                    && [general::FUTEX_WAIT, general::FUTEX_WAIT_BITSET]
                        .map(|wait| wait as i32)
                        .contains(&(request.arguments[1] as i32 & general::FUTEX_CMD_MASK)) =>
            {
                self.stage = SuspendStage::Joining;
            }
            (SuspendPlan::Kernel, SuspendStage::Start) if sleep => {
                self.stage = SuspendStage::Waiting;
            }
            _ => {}
        }
        ControlFlow::Continue(request)
    }

    fn intercept_system_call_response(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        let write = request.number == u64::from(general::__NR_write);
        if write && self.stage == SuspendStage::MainBack && Some(thread) == self.process {
            let mut refused = self.refused_in_step.lock().unwrap();
            if refused.is_none() {
                *refused = Some(process.suspend_tasks(&[thread]).count());
            }
        }
        response
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        match self.stage {
            SuspendStage::MainAway => {
                self.put_back(kernel, cpu, true);
                self.stage = SuspendStage::MainBack;
                true
            }
            SuspendStage::Joining => {
                self.put_back(kernel, cpu, false);
                self.stage = SuspendStage::Done;
                self.done.store(true, std::sync::atomic::Ordering::Relaxed);
                true
            }
            SuspendStage::Waiting => {
                self.one_by_one(kernel, cpu);
                self.whole_process(kernel, cpu);
                self.second_kernel(kernel, cpu);
                self.stage = SuspendStage::Done;
                self.done.store(true, std::sync::atomic::Ordering::Relaxed);
                true
            }
            SuspendStage::Start
            | SuspendStage::SleptOnce
            | SuspendStage::Away
            | SuspendStage::MainBack
            | SuspendStage::Done => false,
        }
    }
}

/// Every order of `N` places: each array of `N` places that names every place once.
fn orders_of<const N: usize>() -> Vec<[usize; N]> {
    let place = |code: usize, at: usize| code / N.pow(at as u32) % N;
    (0..N.pow(N as u32))
        .map(|code| std::array::from_fn(|at| place(code, at)))
        .filter(|order: &[usize; N]| order.iter().collect::<BTreeSet<_>>().len() == N)
        .collect()
}

#[test]
fn tasks_suspended_from_hooks_and_between_steps_go_on_as_the_reference_runs() {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let (address, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
    let theirs = reference::run(&program, &["suspend", &address, &pages]);
    let theirs = String::from_utf8_lossy(&theirs.stdout).into_owned();
    let seeds: Vec<u64> = wcontainers_kernel_test::schedule_seeds().collect();
    let orders = orders_of::<SUSPENDED_THREADS>();
    assert_eq!(orders.len(), (1..=SUSPENDED_THREADS).product::<usize>());
    for plan in [SuspendPlan::Hooks, SuspendPlan::Kernel] {
        for (index, &order) in orders.iter().enumerate() {
            let seed = seeds[index % seeds.len()];
            let wrong = Arc::new(Mutex::new(Vec::new()));
            let refused_in_step = Arc::new(Mutex::new(None));
            let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let suspender = Suspender {
                plan,
                order,
                stage: SuspendStage::Start,
                process: None,
                threads: Vec::new(),
                wrong: wrong.clone(),
                refused_in_step: refused_in_step.clone(),
                done: done.clone(),
            };
            let run = run_embedder("suspend", Box::new(suspender), seed);
            let label = format!("{plan:?}, order {order:?}, seed {seed}");
            assert_eq!(stdout(&run), theirs, "{label}: {:?}", run.counters);
            assert_eq!(*wrong.lock().unwrap(), Vec::<String>::new(), "{label}");
            assert!(done.load(std::sync::atomic::Ordering::Relaxed), "{label}");
            let refused = (plan == SuspendPlan::Hooks).then_some(0);
            assert_eq!(*refused_in_step.lock().unwrap(), refused, "{label}");
            assert_eq!(
                counted(&run, reason::TASK_IN_A_STEP_NOT_SUSPENDED),
                u64::from(plan == SuspendPlan::Hooks),
                "{label}: {:?}",
                run.counters
            );
        }
    }
}

/// How [`Stopper`] takes stopped.c's child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopPlan {
    /// `suspend_process`, then `resume_process` and `resume_tasks`.
    Process,
    /// `suspend_tasks` naming every thread of the child, put back in the reverse order.
    Tasks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopStage {
    Running,
    /// The parent asked to stop the child.
    StopAsked,
    /// The parent, taken at its wait for the stop, is away until the child is taken.
    ParentAway,
    /// The child is away.
    Stopped,
    /// The parent asked to continue the child.
    ContinueAsked,
    Continued,
}

/// Stands in for stopped.c's stop and continue by taking the child off the kernel and
/// putting it back: the parent's `kill` is answered 0, and its wait for the stop is answered with
/// the child once the child is taken (the parent itself is taken at that wait, so the kernel goes
/// idle with the child still waiting in its calls).
struct Stopper {
    plan: StopPlan,
    stage: StopStage,
    parent: Option<TaskId>,
    child: Option<TaskId>,
    threads: Vec<(TaskId, TaskId)>,
    wrong: Arc<Mutex<Vec<String>>>,
}

impl Stopper {
    fn check(&self, what: &str, holds: bool) {
        if !holds {
            self.wrong.lock().unwrap().push(what.to_owned());
        }
    }
}

impl Hooks for Stopper {
    fn on_process_starting(&mut self, process: Process<'_>) {
        self.parent.get_or_insert(process.id());
    }

    fn on_thread_starting(&mut self, process: Process<'_>, thread: TaskId) {
        self.threads.push((process.id(), thread));
    }

    fn intercept_system_call_request(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        if Some(thread) != self.parent {
            return ControlFlow::Continue(request);
        }
        let [first, second, third, ..] = request.arguments;
        let number = request.number;
        let kill = number == u64::from(general::__NR_kill);
        let wait =
            number == u64::from(general::__NR_wait4) && third & u64::from(general::WUNTRACED) != 0;
        match self.stage {
            StopStage::Running if kill && second == u64::from(general::SIGSTOP) => {
                self.child = Some(TaskId(first as u32));
                self.stage = StopStage::StopAsked;
                ControlFlow::Break(Ok(0))
            }
            StopStage::StopAsked if wait => {
                let taken: Vec<_> = process.suspend_tasks(&[thread]).collect();
                self.check("the parent taken at its wait", taken.len() == 1);
                SUSPENDED
                    .with_borrow_mut(|stash| stash.extend(taken.into_iter().map(|(_, task)| task)));
                self.stage = StopStage::ParentAway;
                ControlFlow::Continue(request)
            }
            StopStage::Stopped if wait => {
                ControlFlow::Break(Ok(self.child.expect("the child").0.into()))
            }
            StopStage::Stopped if kill && second == u64::from(general::SIGCONT) => {
                self.stage = StopStage::ContinueAsked;
                ControlFlow::Break(Ok(0))
            }
            _ => ControlFlow::Continue(request),
        }
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        match self.stage {
            StopStage::ParentAway => {
                let parent = SUSPENDED.take();
                let child = self.child.expect("the child");
                let every: Vec<TaskId> = std::iter::once(child)
                    .chain(
                        self.threads
                            .iter()
                            .filter(|(process, _)| *process == child)
                            .map(|&(_, thread)| thread),
                    )
                    .collect();
                let taken: Vec<_> = match self.plan {
                    StopPlan::Process => {
                        let (context, tasks) =
                            kernel.suspend_process(cpu, child).expect("the child");
                        CONTEXT.set(Some(context));
                        tasks.collect()
                    }
                    StopPlan::Tasks => kernel.suspend_tasks(cpu, &every).collect(),
                };
                // A thread that already ended is absent.
                self.check(
                    "the child's threads taken",
                    taken.first().map(|(id, _)| *id) == Some(child)
                        && taken.iter().all(|(id, _)| every.contains(id)),
                );
                SUSPENDED.set(taken.into_iter().map(|(_, task)| task).collect());
                self.check(
                    "the parent put back",
                    kernel.resume_tasks(cpu, parent).next().is_none(),
                );
                self.stage = StopStage::Stopped;
                true
            }
            StopStage::ContinueAsked => {
                if let Some(context) = CONTEXT.take() {
                    self.check(
                        "the child's context put back",
                        kernel
                            .resume_process(context, ExecutionConfig::default())
                            .is_none(),
                    );
                }
                let mut tasks = SUSPENDED.take();
                if self.plan == StopPlan::Tasks {
                    tasks.reverse();
                }
                self.check(
                    "the child's threads put back",
                    kernel.resume_tasks(cpu, tasks).next().is_none(),
                );
                self.stage = StopStage::Continued;
                true
            }
            StopStage::Running
            | StopStage::StopAsked
            | StopStage::Stopped
            | StopStage::Continued => false,
        }
    }
}

#[test]
fn a_task_taken_in_its_call_answers_as_a_stop_on_the_reference_makes_it_answer() {
    let program = wcontainers_kernel_test::guest::build("stopped", &[], &out());
    let theirs = reference::run(&program, &[]);
    let theirs = String::from_utf8_lossy(&theirs.stdout).into_owned();
    for plan in [StopPlan::Process, StopPlan::Tasks] {
        for seed in wcontainers_kernel_test::schedule_seeds() {
            let wrong = Arc::new(Mutex::new(Vec::new()));
            let stopper = Stopper {
                plan,
                stage: StopStage::Running,
                parent: None,
                child: None,
                threads: Vec::new(),
                wrong: wrong.clone(),
            };
            let run = try_guest("stopped", &[], Some(Box::new(stopper)), seed, None)
                .expect("an outcome on our kernel");
            let label = format!("{plan:?}, seed {seed}");
            assert_eq!(stdout(&run), theirs, "{label}: {:?}", run.counters);
            assert_eq!(*wrong.lock().unwrap(), Vec::<String>::new(), "{label}");
        }
    }
}
