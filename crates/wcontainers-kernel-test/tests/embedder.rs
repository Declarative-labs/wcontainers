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
    AccessKind, Errno, Kernel, MemoryError, Process, System, SystemCallResponse, TaskId,
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
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let mut files = wcontainers_kernel_test::image::reference_image().to_vec();
    files.extend(wcontainers_kernel_test::image::host_directory(
        program.parent().expect("a directory"),
        guest_directory_entry(),
    ));
    let path = in_guest_directory("embedder");
    let (buffer, pages) = (format!("{buffer:#x}"), PAGES.to_string());
    let mut command = vec![path.as_str(), mode[0], buffer.as_str(), pages.as_str()];
    command.extend(&mode[1..]);
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
            process.write_memory(first.start, &bytes).expect("writable");
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
/// watched, and each is filled from the saved copy when first touched; a fork's child fills from
/// its parent's copy. A page found changed when it is filled was written past its watch.
#[derive(Default)]
struct Lazy {
    saved: BTreeMap<u32, Arc<BTreeMap<u64, Vec<u8>>>>,
    filled: Arc<Mutex<u64>>,
    written_past: Arc<Mutex<Vec<u64>>>,
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
        _: TaskId,
        address: u64,
        _: AccessKind,
    ) {
        let page = page_of(address);
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
