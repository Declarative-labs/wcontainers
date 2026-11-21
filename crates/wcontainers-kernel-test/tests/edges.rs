//! The embedder's hooks at their edges (guest/edges.c): a signal sent while a thread waits on a
//! watch outside a call, a thread taken and put back during that wait, which fatal signals are
//! announced, shared memory cut and joined in every way, a reader of a file mapped while its only
//! writer is away, and the memory ids of children; compared with the reference.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::{ControlFlow, Range};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wcontainers_abi::{Cpu, MemoryId, PAGE_SIZE, SystemCallRequest};
use wcontainers_kernel_linux::linux::general;
use wcontainers_kernel_linux::{
    AccessKind, ExecutionConfig, Kernel, MemoryObject, Process, ProcessContext, SignalInformation,
    SuspendedTask, SystemCallResponse, TaskId, WatchesFor,
};
use wcontainers_kernel_test::driver::{self, Run};
use wcontainers_kernel_test::guest_directory_entry;
use wcontainers_kernel_test::hooks::Hooks;
use wcontainers_kernel_test::reference::{self, ROOT_DIRECTORY, in_guest_directory};

const PROGRAM: &str = "edges";
/// edges.c's `EDGES_BASE`: where its mappings go.
const BASE: u64 = 0x3600_0000;
/// edges.c's `EDGES_SENT_VALUE`.
const SENT_VALUE: i32 = 21;

fn out() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(PROGRAM)
}

fn program() -> PathBuf {
    wcontainers_kernel_test::guest::build(PROGRAM, &[], &out())
}

fn theirs(arguments: &[&str]) -> String {
    String::from_utf8_lossy(&reference::run(&program(), arguments).stdout).into_owned()
}

fn ours(arguments: &[&str], hooks: Box<dyn Hooks>, seed: u64) -> Run {
    let program = program();
    let mut files = wcontainers_kernel_test::image::reference_image().to_vec();
    files.extend(wcontainers_kernel_test::image::host_directory(
        program.parent().expect("a directory"),
        guest_directory_entry(),
    ));
    let path = in_guest_directory(PROGRAM);
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
        root_image: None,
        address_space: wcontainers_kernel_test::standin::SPACE_32_BIT,
    };
    let prepared = driver::Prepared {
        hooks: Some(hooks),
        ..driver::prepare(&container)
    };
    driver::run_prepared(prepared, &container, seed, Default::default())
        .expect("an outcome on our kernel")
}

fn stdout(run: &Run) -> String {
    String::from_utf8_lossy(&run.outcome.stdout).into_owned()
}

// ---------------------------------------------------------------- a signal during a watch wait

/// Watches the page edges.c's worker writes at the main thread's first mark, sends the worker
/// SIGUSR1 at the second and ends the watch at the third, noting what happens in order.
#[derive(Default)]
struct Alone {
    marks: usize,
    worker: Option<TaskId>,
    log: Arc<Mutex<Vec<String>>>,
}

impl Hooks for Alone {
    fn on_thread_starting(&mut self, _: Process<'_>, thread: TaskId) {
        self.worker.get_or_insert(thread);
    }

    fn intercept_system_call_request(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        let mut log = self.log.lock().unwrap();
        if Some(thread) == self.worker {
            log.push(format!("worker call {}", request.number));
        } else if request.number == u64::from(general::__NR_getppid) {
            let page = BASE..BASE + PAGE_SIZE;
            match self.marks {
                0 => {
                    process.watch_memory(page, WatchesFor::Writes);
                    log.push("watched".to_owned());
                }
                1 => {
                    let worker = self.worker.expect("the worker");
                    process.send_signal(worker, queued(general::SIGUSR1, SENT_VALUE));
                    log.push("sent".to_owned());
                }
                _ => {
                    process.watch_memory(page, WatchesFor::None);
                    log.push("unwatched".to_owned());
                }
            }
            self.marks += 1;
        }
        ControlFlow::Continue(request)
    }

    fn on_watched_memory_accessing(&mut self, _: Process<'_>, _: TaskId, _: u64, _: AccessKind) {
        self.log.lock().unwrap().push("accessed".to_owned());
    }

    fn on_signal_delivering(&mut self, _: Process<'_>, _: TaskId, signal: &SignalInformation) {
        // SAFETY: the kernel builds every record it hands over from all of its bytes.
        let number = unsafe { signal.__bindgen_anon_1.__bindgen_anon_1.si_signo };
        self.log
            .lock()
            .unwrap()
            .push(format!("delivering {number}"));
    }
}

/// A record of `signal` queued with `value` from outside the container (no sender process).
fn queued(signal: u32, value: i32) -> SignalInformation {
    // SAFETY: every member of the record is made of integers, for which zero is valid.
    let mut record: SignalInformation = unsafe { std::mem::zeroed() };
    // SAFETY: a record built from zero has every member's bytes set.
    let header = unsafe { &mut record.__bindgen_anon_1.__bindgen_anon_1 };
    header.si_signo = signal as i32;
    header.si_code = general::SI_QUEUE;
    header._sifields._rt._sigval.sival_int = value;
    record
}

#[test]
fn a_thread_alone_acts_on_a_signal_sent_during_its_watch_wait_once_the_watches_change() {
    let theirs = theirs(&["alone", "self"]);
    println!("reference:\n{theirs}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let hooks = Alone::default();
        let log = hooks.log.clone();
        let run = ours(&["alone", "embedder"], Box::new(hooks), seed);
        let log = log.lock().unwrap().clone();
        assert_eq!(stdout(&run), theirs, "seed {seed}: {log:?}");
        // The worker meets the watch and waits; the signal waits with it, and is acted on when
        // the watch ends, before the worker makes any call.
        let unwatched = log.iter().position(|event| event == "unwatched");
        let unwatched = unwatched.expect("the watch ends");
        let before: Vec<&str> = log[..unwatched]
            .iter()
            .map(String::as_str)
            .filter(|event| !event.starts_with("worker call"))
            .collect();
        assert_eq!(
            before,
            ["watched", "accessed", "sent"],
            "seed {seed}: {log:?}"
        );
        let delivering = format!("delivering {}", general::SIGUSR1);
        assert_eq!(
            log.get(unwatched + 1),
            Some(&delivering),
            "seed {seed}: {log:?}"
        );
    }
}

// ---------------------------------------------------------------- taken during a watch wait

/// Watches the region edges.c's main thread names in its mark (its address and size); once the worker has met the watch and the
/// kernel is idle, takes the worker, puts it back, and ends the watch once the worker meets it again.
#[derive(Default)]
struct TakenWhileWaiting {
    process: Option<TaskId>,
    worker: Option<TaskId>,
    region: Range<u64>,
    accesses: usize,
    stage: usize,
    log: Arc<Mutex<Vec<String>>>,
}

impl Hooks for TakenWhileWaiting {
    fn on_thread_starting(&mut self, process: Process<'_>, thread: TaskId) {
        self.process.get_or_insert(process.id());
        self.worker.get_or_insert(thread);
    }

    fn intercept_system_call_request(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        if request.number == u64::from(general::__NR_getppid) {
            let [start, size, ..] = request.arguments;
            self.region = start..start + size;
            process.watch_memory(self.region.clone(), WatchesFor::Writes);
            self.log.lock().unwrap().push("watched".to_owned());
        } else if Some(thread) == self.worker {
            let mut log = self.log.lock().unwrap();
            log.push(format!("worker call {}", request.number));
        }
        ControlFlow::Continue(request)
    }

    fn on_watched_memory_accessing(&mut self, _: Process<'_>, _: TaskId, _: u64, _: AccessKind) {
        self.accesses += 1;
        self.log.lock().unwrap().push("accessed".to_owned());
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        // The worker waits on the watch before it is taken, and again once it is back.
        let waiting = if self.stage < 2 { 1 } else { 2 };
        if self.accesses < waiting {
            return false;
        }
        let worker = self.worker.expect("the worker");
        let mut log = self.log.lock().unwrap();
        match self.stage {
            0 => {
                let taken: Vec<SuspendedTask> = kernel
                    .suspend_tasks(cpu, &[worker])
                    .map(|(_, task)| task)
                    .collect();
                log.push(format!("taken {}", taken.len()));
                TAKEN.with_borrow_mut(|held| held.extend(taken));
            }
            1 => {
                let back = kernel.resume_tasks(cpu, TAKEN.take()).count();
                log.push(format!("handed back {back}"));
            }
            2 => {
                let pid = self.process.expect("the process");
                let mut process = kernel.process(cpu, pid).expect("the process");
                process.watch_memory(self.region.clone(), WatchesFor::None);
                log.push("unwatched".to_owned());
            }
            _ => return false,
        }
        self.stage += 1;
        true
    }
}

/// edges.c `taken` with `how`: the worker taken and put back while it waits outside a call answers
/// its later calls as the reference does, with no interruption.
fn taken_while_waiting(how: &str) {
    let theirs = theirs(&["taken", how]);
    println!("reference:\n{theirs}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let hooks = TakenWhileWaiting::default();
        let log = hooks.log.clone();
        let run = ours(&["taken", how], Box::new(hooks), seed);
        TAKEN.take();
        let log = log.lock().unwrap().clone();
        let events: Vec<&str> = log
            .iter()
            .map(String::as_str)
            .filter(|event| !event.starts_with("worker call"))
            .collect();
        assert_eq!(
            events,
            [
                "watched",
                "accessed",
                "taken 1",
                "handed back 0",
                "accessed",
                "unwatched"
            ],
            "seed {seed}: {log:?}"
        );
        assert_eq!(stdout(&run), theirs, "seed {seed}: {log:?}");
    }
}

#[test]
fn a_task_taken_while_its_store_waits_on_a_watch_is_not_interrupted_later() {
    taken_while_waiting("store");
}

#[test]
fn a_task_taken_while_its_signal_frame_waits_on_a_watch_is_not_interrupted_later() {
    taken_while_waiting("frame");
}

// ---------------------------------------------------------------- fatal signals announced

/// The number of every signal announced, in order.
#[derive(Default)]
struct Announced(Arc<Mutex<Vec<i32>>>);

impl Hooks for Announced {
    fn on_signal_delivering(&mut self, _: Process<'_>, _: TaskId, signal: &SignalInformation) {
        // SAFETY: the kernel builds every record it hands over from all of its bytes.
        let number = unsafe { signal.__bindgen_anon_1.__bindgen_anon_1.si_signo };
        self.0.lock().unwrap().push(number);
    }
}

#[test]
fn a_signal_that_dumps_core_is_announced_and_one_that_only_ends_is_not() {
    let theirs = theirs(&["fatal"]);
    println!("reference:\n{theirs}");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let hooks = Announced::default();
        let announced = hooks.0.clone();
        let run = ours(&["fatal"], Box::new(hooks), seed);
        assert_eq!(stdout(&run), theirs, "seed {seed}");
        let cores =
            [general::SIGQUIT, general::SIGSEGV, general::SIGABRT].map(|signal| signal as i32);
        assert_eq!(*announced.lock().unwrap(), cores, "seed {seed}");
    }
}

// ---------------------------------------------------------------- shared memory

/// One announcement's place: the process, then the page, mapped to the source and its offset.
type Pages = BTreeMap<(TaskId, u64), (MemoryObject, u64)>;

/// What a sharing test sees: the pages announced, the starts not yet ended, each step's pages.
#[derive(Default)]
struct Sharing {
    pages: Pages,
    starts: Vec<(TaskId, MemoryObject, u64, Range<u64>)>,
    steps: Vec<(TaskId, BTreeMap<u64, u64>)>,
}

impl Sharing {
    fn start(&mut self, process: TaskId, source: MemoryObject, offset: u64, range: Range<u64>) {
        for page in range.clone().step_by(PAGE_SIZE as usize) {
            let place = (source, offset + (page - range.start));
            let before = self.pages.insert((process, page), place);
            assert_eq!(before, None, "{process:?} {page:#x} announced twice");
        }
        self.starts.push((process, source, offset, range));
    }

    /// An end must lie inside one start of the same process and source, at its offsets.
    fn end(&mut self, process: TaskId, source: MemoryObject, offset: u64, range: Range<u64>) {
        for page in range.clone().step_by(PAGE_SIZE as usize) {
            let place = (source, offset + (page - range.start));
            let before = self.pages.remove(&(process, page));
            assert_eq!(before, Some(place), "{process:?} {page:#x} ended");
        }
        let shift = range.start.wrapping_sub(offset);
        let within = self.starts.iter().position(|(owner, from, at, start)| {
            *owner == process
                && *from == source
                && start.start.wrapping_sub(*at) == shift
                && start.start <= range.start
                && range.end <= start.end
        });
        let Some(within) = within else {
            panic!(
                "{process:?} end {offset:#x} {range:x?} lies in no one start: {:x?}",
                self.starts
            );
        };
        let (_, _, at, start) = self.starts.remove(within);
        if start.start < range.start {
            self.starts
                .push((process, source, at, start.start..range.start));
        }
        if range.end < start.end {
            let after = at + (range.end - start.start);
            self.starts
                .push((process, source, after, range.end..start.end));
        }
    }

    fn step(&mut self, process: TaskId) {
        let mine = self
            .pages
            .iter()
            .filter(|((owner, _), _)| *owner == process)
            .map(|((_, page), (_, offset))| (*page, *offset))
            .collect();
        self.steps.push((process, mine));
    }
}

thread_local! {
    /// The writer's context and tasks while they are away (the kernel's own values).
    static CONTEXT: RefCell<Option<ProcessContext>> = const { RefCell::new(None) };
    static TAKEN: RefCell<Vec<SuspendedTask>> = const { RefCell::new(Vec::new()) };
}

/// Where the writer plan is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Writer {
    /// Nothing is taken (the `ends` mode).
    #[default]
    Never,
    /// The parent waits for its child, which sleeps: the parent is taken off the kernel then.
    Waiting,
    /// The parent is away until the child has ended.
    Away,
    Back,
}

/// Notes every sharing announcement and the pages each process was told at each `sched_yield`;
/// with the writer plan, takes the parent off the kernel while its child maps the file.
#[derive(Default)]
struct SharingRecorder {
    seen: Arc<Mutex<Sharing>>,
    plan: Writer,
    processes: Vec<TaskId>,
    child_sleeping: bool,
    child_ended: bool,
}

impl Hooks for SharingRecorder {
    fn on_process_starting(&mut self, process: Process<'_>) {
        self.processes.push(process.id());
    }

    fn on_process_exiting(&mut self, process: Process<'_>) {
        if self.processes.get(1) == Some(&process.id()) {
            self.child_ended = true;
        }
    }

    fn intercept_system_call_request(
        &mut self,
        process: Process<'_>,
        _: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        if request.number == u64::from(general::__NR_sched_yield) {
            self.seen.lock().unwrap().step(process.id());
        }
        if request.number == u64::from(general::__NR_nanosleep)
            && self.processes.get(1) == Some(&process.id())
        {
            self.child_sleeping = true;
        }
        ControlFlow::Continue(request)
    }

    fn on_shared_memory_mapping(
        &mut self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        self.seen
            .lock()
            .unwrap()
            .start(process.id(), source, offset, range);
    }

    fn on_shared_memory_unmapped(
        &mut self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        self.seen
            .lock()
            .unwrap()
            .end(process.id(), source, offset, range);
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        match self.plan {
            Writer::Waiting if self.child_sleeping => {
                let parent = self.processes[0];
                let (context, tasks) = kernel.suspend_process(cpu, parent).expect("the parent");
                CONTEXT.set(Some(context));
                TAKEN.set(tasks.map(|(_, task)| task).collect());
                self.plan = Writer::Away;
                true
            }
            Writer::Away if self.child_ended => {
                let context = CONTEXT.take().expect("the parent's context");
                let back = kernel.resume_process(context, ExecutionConfig::default());
                assert!(back.is_none(), "the parent's context is put back");
                assert_eq!(kernel.resume_tasks(cpu, TAKEN.take()).count(), 0);
                self.plan = Writer::Back;
                true
            }
            _ => false,
        }
    }
}

/// edges.c's steps: each name, and the pages /proc/self/maps showed in the region then (page,
/// offset in the source).
fn steps_of(output: &str) -> Vec<(String, BTreeMap<u64, u64>)> {
    let mut steps: Vec<(String, BTreeMap<u64, u64>)> = Vec::new();
    for line in output.lines() {
        if let Some(name) = line.strip_prefix("step ") {
            steps.push((name.to_owned(), BTreeMap::new()));
            continue;
        }
        let words: Vec<&str> = line.split(' ').collect();
        let [range, permissions, offset] = words.as_slice() else {
            continue;
        };
        let (Some((start, end)), Some(steps)) = (range.split_once('-'), steps.last_mut()) else {
            continue;
        };
        // Only shared mappings are announced.
        if !permissions.ends_with('s') {
            continue;
        }
        let hex = |text: &str| u64::from_str_radix(text, 16).expect("a number");
        let (start, end, offset) = (hex(start), hex(end), hex(offset));
        for page in (start..end).step_by(PAGE_SIZE as usize) {
            steps.1.insert(page, offset + (page - start));
        }
    }
    steps
}

/// The lines of edges.c's output that are not lines of /proc/self/maps.
fn told(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| !line.starts_with(|first: char| first.is_ascii_hexdigit()))
        .collect()
}

/// Runs `mode` with `plan` and checks, at each step, that the pages each process was told it maps
/// equal those its /proc/self/maps showed in the reference, and that everything told ends.
fn sharing_as_the_reference(mode: &str, plan: Writer) {
    let theirs = theirs(&[mode]);
    println!("reference:\n{theirs}");
    let steps = steps_of(&theirs);
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let recorder = SharingRecorder {
            plan,
            ..SharingRecorder::default()
        };
        let seen = recorder.seen.clone();
        let run = ours(&[mode], Box::new(recorder), seed);
        let ours = stdout(&run);
        println!("seed {seed}:\n{ours}");
        // Our /proc/self/maps joins two mappings side by side that Linux keeps apart (recorded in
        // the status file); what each step shows is compared page by page below.
        assert_eq!(told(&ours), told(&theirs), "seed {seed}:\n{ours}");
        assert_eq!(steps_of(&ours), steps, "seed {seed}:\n{ours}");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.steps.len(), steps.len(), "seed {seed}");
        for ((name, shown), (process, told)) in steps.iter().zip(&seen.steps) {
            assert_eq!(told, shown, "seed {seed}, step {name} ({process:?})");
        }
        assert!(seen.pages.is_empty(), "seed {seed}: {:x?}", seen.pages);
        assert!(seen.starts.is_empty(), "seed {seed}: {:x?}", seen.starts);
    }
}

#[test]
fn every_end_of_shared_memory_lies_in_one_start_and_each_step_matches_proc_maps() {
    sharing_as_the_reference("ends", Writer::Never);
}

#[test]
fn a_read_only_mapper_is_announced_while_the_only_writer_is_away() {
    sharing_as_the_reference("writer", Writer::Waiting);
}

// ---------------------------------------------------------------- memory ids

/// A process at its start: itself, its parent and its memory; or a program's start (no parent).
type Started = (TaskId, Option<TaskId>, MemoryId);

#[derive(Default)]
struct MemoryIds(Arc<Mutex<Vec<Started>>>);

impl Hooks for MemoryIds {
    fn on_process_starting(&mut self, process: Process<'_>) {
        let parent = process.info().parent();
        let started = (process.id(), parent, process.memory_id());
        self.0.lock().unwrap().push(started);
    }

    fn on_program_starting(&mut self, process: Process<'_>) {
        let started = (process.id(), None, process.memory_id());
        self.0.lock().unwrap().push(started);
    }
}

#[test]
fn a_child_shares_its_parents_memory_id_only_while_it_shares_the_memory() {
    let theirs = theirs(&["memory-id"]);
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let ids = MemoryIds::default();
        let seen = ids.0.clone();
        let run = ours(&["memory-id"], Box::new(ids), seed);
        assert_eq!(stdout(&run), theirs, "seed {seed}");
        let seen = seen.lock().unwrap().clone();
        // The first process and its program, a forked child, a vfork child and the program it
        // runs, a child cloned with CLONE_VM.
        let [
            (first, None, memory),
            (program, None, program_memory),
            (_, Some(forker), forked),
            (vforked, Some(vforker), vfork_memory),
            (runs, None, new_memory),
            (_, Some(cloner), cloned),
        ] = seen[..]
        else {
            panic!("seed {seed}: {seen:?}");
        };
        let label = format!("seed {seed}: {seen:?}");
        assert!(program == first && program_memory == memory, "{label}");
        assert!(forker == first && forked != memory, "{label}");
        assert!(vforker == first && vfork_memory == memory, "{label}");
        let earlier = [memory, forked];
        assert!(runs == vforked && !earlier.contains(&new_memory), "{label}");
        assert!(cloner == first && cloned == memory, "{label}");
    }
}
