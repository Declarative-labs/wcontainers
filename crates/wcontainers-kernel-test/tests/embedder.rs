//! The embedder's interface: the life-cycle hooks and their order, task ids handed out by the
//! embedder, watches met before a call has any effect, the record of written pages, and lazy
//! memory built from the two primitives running the shell's scripts as the reference does.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use wcontainers_abi::{Cpu, PAGE_SIZE};
use wcontainers_kernel::{Access, Kernel, Process, TaskId, Watch, reason};
use wcontainers_kernel_test::driver::{self, Run};
use wcontainers_kernel_test::hooks::{Defaults, Hooks};
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
    run_embedder_below(&[mode], hooks, seed, None)
}

/// As [`run_embedder`], with more arguments and the executor's ceiling given at start.
fn run_embedder_below(
    mode: &[&str],
    hooks: Box<dyn Hooks>,
    seed: u64,
    ceiling: Option<u64>,
) -> Run {
    let program = wcontainers_kernel_test::guest::build("embedder", &[], &out());
    let mut files = wcontainers_kernel_test::image::reference_image().to_vec();
    files.extend(wcontainers_kernel_test::image::host_directory(
        program.parent().expect("a directory"),
        guest_directory_entry(),
    ));
    let path = in_guest_directory("embedder");
    let (buffer, pages) = (format!("{BUFFER:#x}"), PAGES.to_string());
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
        .expect("an outcome on our kernel")
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
    seen: Rc<RefCell<Vec<Seen>>>,
    depth: Rc<RefCell<u32>>,
    pool: Option<(u32, Vec<u32>)>,
    allocated: Rc<RefCell<Vec<u32>>>,
}

impl Recorder {
    /// Enters a hook: none may be running already. Every operation of the handle runs, and
    /// none may announce another hook.
    fn enter(&self, process: &mut Process<'_>, seen: Seen) {
        assert_eq!(
            *self.depth.borrow(),
            0,
            "a hook nested in another: {seen:?}"
        );
        *self.depth.borrow_mut() += 1;
        let info = process.info();
        assert!(info.memory_mapped() > 0, "{seen:?}: memory mapped");
        let _ = (info.ceiling_distance(), info.compute_used());
        let _ = process.execution_config();
        let changed = process.memory_changed();
        if let Some(first) = changed.first() {
            let bytes = process
                .memory_read(first.start..first.start + 1)
                .expect("readable");
            process.memory_write(first.start, &bytes).expect("writable");
            process.memory_watch(first.clone(), Watch::None);
        }
        assert!(
            process
                .memory_watch_thread(TaskId(0), buffer(), Watch::All)
                .is_err()
        );
        self.seen.borrow_mut().push(seen);
        *self.depth.borrow_mut() -= 1;
    }
}

impl Hooks for Recorder {
    fn task_id_allocate(&mut self, previous: TaskId) -> TaskId {
        let Some((next, freed)) = &mut self.pool else {
            return wcontainers_kernel::task_id_after(previous);
        };
        let id = freed.pop().unwrap_or_else(|| {
            *next += 1;
            *next
        });
        self.allocated.borrow_mut().push(id);
        TaskId(id)
    }

    fn task_id_free(&mut self, id: TaskId) {
        assert_eq!(*self.depth.borrow(), 0, "a task id freed inside a hook");
        if let Some((_, freed)) = &mut self.pool {
            assert!(!freed.contains(&id.0), "{id:?} freed twice");
            freed.push(id.0);
        }
        self.seen.borrow_mut().push(Seen::Free(id.0));
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
            seen.borrow()
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
        assert_eq!(*seen.borrow(), expected, "seed {seed}");
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
        assert_eq!(*allocated.borrow(), [1001, 1002, 1002], "seed {seed}");
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
        assert_eq!(*seen.borrow(), expected, "seed {seed}");
    }
}

/// Records each `on_process_outgrown`: the bytes asked for and the distance left to the ceiling.
struct Outgrown(Rc<RefCell<Vec<(u64, u64)>>>);

impl Hooks for Outgrown {
    fn on_process_outgrown(&mut self, process: Process<'_>, bytes: u64) {
        self.0
            .borrow_mut()
            .push((bytes, process.info().ceiling_distance()));
    }
}

/// The piece embedder.c's `outgrow` maps at a time, and the ceiling the test gives.
const PIECE: u64 = 64 << 20;
const CEILING: u64 = 1 << 30;

#[test]
fn a_request_past_the_ceiling_is_announced_and_fails_as_out_of_memory() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let run = run_embedder_below(
            &["outgrow"],
            Box::new(Outgrown(seen.clone())),
            seed,
            Some(CEILING),
        );
        let text = stdout(&run);
        let words: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(words[0], "outgrown", "{text}");
        assert_eq!(words[2], "ENOMEM", "{text}");
        let pieces: u64 = words[1].parse().expect("a count");
        assert!(pieces > 0 && pieces * PIECE < CEILING, "{text}");
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1, "{seen:?}");
        let (bytes, distance) = seen[0];
        assert_eq!(bytes, PIECE);
        assert!(distance < PIECE, "{distance}");
        assert_eq!(counted(&run, reason::PROCESS_OUTGROWN), 1);
        // With no ceiling below the address space's end, nothing is announced.
        let seen_without = Rc::new(RefCell::new(Vec::new()));
        let run = run_embedder("outgrow", Box::new(Outgrown(seen_without.clone())), seed);
        assert!(seen_without.borrow().is_empty());
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
        let seen = Rc::new(RefCell::new(Vec::new()));
        let run = run_embedder_below(
            &["fixed", &address],
            Box::new(Outgrown(seen.clone())),
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
            seen.borrow()
                .iter()
                .map(|(bytes, _)| *bytes)
                .collect::<Vec<_>>(),
            [PAGE_SIZE, PAGE_SIZE]
        );
        assert_eq!(counted(&run, reason::PROCESS_OUTGROWN), 2);
        assert_eq!(counted(&run, reason::ADDRESS_ABOVE_SPACE), 0);
        // Below the address space's end and with no ceiling, both are mapped, as on Linux.
        let run = run_embedder_below(&["fixed", &address], Box::new(Defaults), seed, None);
        assert_eq!(stdout(&run), "fixed mapped\nnoreplace mapped\n");
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
        assert_eq!(*seen.borrow(), expected, "seed {seed}");
    }
}

/// What a watching test saw at each hook: the process, the address, how, and whether the
/// watched page still held what it held when the process started.
type Touches = Rc<RefCell<Vec<(u32, u64, Access, bool)>>>;

/// Watches the buffer's writes in every process from its start; meets each watch inside the
/// hook, or `later`, between steps through `Kernel::process`.
struct Watcher {
    later: bool,
    touches: Touches,
    pending: Vec<(TaskId, u64)>,
    at_start: BTreeMap<u32, Vec<u8>>,
}

impl Watcher {
    fn new(later: bool, touches: Touches) -> Self {
        Self {
            later,
            touches,
            pending: Vec::new(),
            at_start: BTreeMap::new(),
        }
    }
}

impl Hooks for Watcher {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        // The first process maps the buffer only once it runs.
        if let Ok(bytes) = process.memory_read(buffer()) {
            self.at_start.insert(process.id().0, bytes);
            process.memory_watch(buffer(), Watch::Writes);
        }
    }

    fn on_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        access: Access,
    ) {
        let page = page_of(address);
        let now = process
            .memory_read(page.clone())
            .expect("the buffer is mapped");
        let offset = (page.start - BUFFER) as usize;
        let unchanged = self.at_start[&process.id().0][offset..offset + now.len()] == now[..];
        self.touches
            .borrow_mut()
            .push((process.id().0, address, access, unchanged));
        if self.later {
            self.pending.push((process.id(), address));
        } else {
            process.memory_watch(page, Watch::None);
        }
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let pending = std::mem::take(&mut self.pending);
        for (id, address) in &pending {
            let mut process = kernel.process(cpu, *id).expect("the process waits");
            process.memory_watch(page_of(*address), Watch::None);
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
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let plain = run_embedder("calls", Box::new(Defaults), seed);
        assert_eq!(plain.outcome.exit_code, 0, "{:?}", plain.outcome);
        let calls = calls_made(&plain);
        assert_eq!(
            stdout(&plain).lines().count(),
            calls + 2,
            "{}",
            stdout(&plain)
        );
        for later in [false, true] {
            let touches = Touches::default();
            let run = run_embedder(
                "calls",
                Box::new(Watcher::new(later, touches.clone())),
                seed,
            );
            assert_eq!(stdout(&run), stdout(&plain), "seed {seed}, later {later}");
            let touches = touches.borrow();
            let processes: BTreeSet<u32> = touches.iter().map(|touch| touch.0).collect();
            assert_eq!(
                processes.len(),
                calls,
                "seed {seed}, later {later}: {touches:?}"
            );
            for (id, address, access, unchanged) in touches.iter() {
                assert!(buffer().contains(address), "{id}: {address:#x}");
                assert_eq!(*access, Access::Write);
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
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let touches = Touches::default();
        let run = run_embedder("span", Box::new(NextPage(touches.clone())), seed);
        assert_eq!(run.outcome.exit_code, 0, "{:?}", run.outcome);
        assert!(
            touches.borrow().is_empty(),
            "seed {seed}: {:?}",
            touches.borrow()
        );
    }
}

/// Keeps a record of written pages and watches the buffer's third page in every child.
struct NextPage(Touches);

impl Hooks for NextPage {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            process.memory_changed();
            process.memory_watch(page_of(BUFFER + 2 * PAGE_SIZE), Watch::Writes);
        }
    }

    fn on_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        access: Access,
    ) {
        self.0
            .borrow_mut()
            .push((process.id().0, address, access, true));
        process.memory_watch(page_of(address), Watch::None);
    }
}

/// Asks for the written pages when a child starts and again when it ends.
struct Changes(Rc<RefCell<Vec<std::ops::Range<u64>>>>);

impl Hooks for Changes {
    fn on_process_starting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            process.memory_changed();
        }
    }

    fn on_process_exiting(&mut self, mut process: Process<'_>) {
        if process.info().parent().is_some() {
            *self.0.borrow_mut() = process.memory_changed();
        }
    }
}

#[test]
fn the_pages_written_are_exactly_those_a_program_wrote() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let changed = Rc::new(RefCell::new(Vec::new()));
        let run = run_embedder("pages", Box::new(Changes(changed.clone())), seed);
        assert_eq!(run.outcome.exit_code, 0, "{:?}", run.outcome);
        let in_buffer: Vec<u64> = changed
            .borrow()
            .iter()
            .flat_map(|range| (range.start..range.end).step_by(PAGE_SIZE as usize))
            .filter(|page| buffer().contains(page))
            .map(|page| (page - BUFFER) / PAGE_SIZE)
            .collect();
        assert_eq!(in_buffer, [0, 2, 5, 7], "seed {seed}");
    }
}

/// Lazy memory from the two primitives: a new program's writable pages are saved, cleared and
/// watched, and each is filled from the saved copy when first touched; a fork's child fills from
/// its parent's copy. A page found changed when it is filled was written past its watch.
#[derive(Default)]
struct Lazy {
    saved: BTreeMap<u32, Rc<BTreeMap<u64, Vec<u8>>>>,
    filled: Rc<RefCell<u64>>,
    written_past: Rc<RefCell<Vec<u64>>>,
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
        for range in process.memory_changed() {
            for page in (range.start..range.end).step_by(PAGE_SIZE as usize) {
                let bytes = process
                    .memory_read(page..page + PAGE_SIZE)
                    .expect("a mapped page");
                process.memory_write(page, &zeros).expect("a mapped page");
                saved.insert(page, bytes);
            }
            process.memory_watch(range, Watch::All);
        }
        self.saved.insert(process.id().0, Rc::new(saved));
    }

    fn on_memory_accessing(
        &mut self,
        mut process: Process<'_>,
        _: TaskId,
        address: u64,
        _: Access,
    ) {
        let page = page_of(address);
        let saved = &self.saved[&process.id().0];
        let bytes = &saved[&page.start];
        let now = process.memory_read(page.clone()).expect("a mapped page");
        if now.iter().any(|byte| *byte != 0) {
            self.written_past.borrow_mut().push(page.start);
        }
        process
            .memory_write(page.start, bytes)
            .expect("a mapped page");
        process.memory_watch(page, Watch::None);
        *self.filled.borrow_mut() += 1;
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
            assert!(*filled.borrow() > 0, "{label}: nothing was filled");
            let undescribed: Vec<_> = ours
                .counters
                .iter()
                .filter(|counter| counter.reason == reason::COPY_UNDESCRIBED)
                .collect();
            assert!(undescribed.is_empty(), "{label}: {undescribed:?}");
            assert!(
                written_past.borrow().is_empty(),
                "{label}: written past the watch: {:x?}",
                written_past.borrow()
            );
        }
    }
}
