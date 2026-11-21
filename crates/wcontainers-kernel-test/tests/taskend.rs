//! Tasks taken off the kernel whose process ends or runs a new program while they are away end as
//! stopped threads end in Linux (guest/taskend.c): what the program prints is the reference's,
//! where nothing is taken, and the task is handed back when it is put back. The end of a copy of a
//! context makes no last stores for the tasks that go on with the original.

use std::cell::RefCell;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wcontainers_abi::{Cpu, SystemCallRequest};
use wcontainers_kernel::linux::general;
use wcontainers_kernel::{
    ExecutionConfig, Kernel, Process, ProcessContext, SuspendedTask, SystemCallResponse, TaskId,
};
use wcontainers_kernel_test::driver::{self, Run};
use wcontainers_kernel_test::guest_directory_entry;
use wcontainers_kernel_test::hooks::Hooks;
use wcontainers_kernel_test::reference::{self, ROOT_DIRECTORY, in_guest_directory};

const PROGRAM: &str = "taskend";

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

thread_local! {
    /// The tasks the test holds while they are away (they are the kernel's own values).
    static TAKEN: RefCell<Vec<SuspendedTask>> = const { RefCell::new(Vec::new()) };
    /// A copy of a context the test puts on the kernel, and the original it holds.
    static COPY: RefCell<Option<ProcessContext>> = const { RefCell::new(None) };
    static ORIGINAL: RefCell<Option<ProcessContext>> = const { RefCell::new(None) };
}

/// Which task the test takes at the mark (`getppid`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Take {
    /// The thread that makes the call, before it.
    Caller,
    /// The other thread of the caller's process.
    Other,
}

#[derive(Debug, Default)]
struct Seen {
    taken: Vec<TaskId>,
    /// Every number freed, in order.
    freed: Vec<TaskId>,
    /// The numbers freed before the test put its tasks back.
    freed_before_back: Option<Vec<TaskId>>,
    handed_back: usize,
}

/// Takes a task at the first mark, and puts it back when the kernel is idle after a program has
/// started or a process has exited since.
struct Script {
    take: Take,
    threads: Vec<(TaskId, TaskId)>,
    marked: bool,
    changed: bool,
    seen: Arc<Mutex<Seen>>,
}

impl Hooks for Script {
    fn free_task_id(&mut self, id: TaskId) {
        self.seen.lock().unwrap().freed.push(id);
    }

    fn on_thread_starting(&mut self, process: Process<'_>, thread: TaskId) {
        self.threads.push((process.id(), thread));
    }

    fn on_program_starting(&mut self, _: Process<'_>) {
        self.changed |= self.marked;
    }

    fn on_process_exiting(&mut self, _: Process<'_>) {
        self.changed |= self.marked;
    }

    fn intercept_system_call_request(
        &mut self,
        mut process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        if request.number != u64::from(general::__NR_getppid) || self.marked {
            return ControlFlow::Continue(request);
        }
        self.marked = true;
        let id = process.id();
        let target = match self.take {
            Take::Caller => thread,
            Take::Other => self
                .threads
                .iter()
                .find(|(owner, other)| *owner == id && *other != thread)
                .map(|(_, other)| *other)
                .expect("the other thread"),
        };
        let taken: Vec<SuspendedTask> = process
            .suspend_tasks(&[target])
            .map(|(_, task)| task)
            .collect();
        assert_eq!(taken.len(), 1, "{target:?} is taken");
        self.seen.lock().unwrap().taken.push(target);
        TAKEN.with_borrow_mut(|held| held.extend(taken));
        ControlFlow::Continue(request)
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        if !self.changed || TAKEN.with_borrow(Vec::is_empty) {
            return false;
        }
        let mut seen = self.seen.lock().unwrap();
        seen.freed_before_back = Some(seen.freed.clone());
        seen.handed_back += kernel.resume_tasks(cpu, TAKEN.take()).count();
        true
    }
}

/// Runs `arguments` under every seed with a task taken at the mark, asks that the output is the
/// reference's, and gives what the test saw.
fn as_the_reference(take: Take, arguments: &[&str]) -> Vec<Seen> {
    let theirs = theirs(arguments);
    wcontainers_kernel_test::schedule_seeds()
        .map(|seed| {
            let seen = Arc::new(Mutex::new(Seen::default()));
            let script = Script {
                take,
                threads: Vec::new(),
                marked: false,
                changed: false,
                seen: seen.clone(),
            };
            let run = ours(arguments, Box::new(script), seed);
            TAKEN.take();
            let seen = std::mem::take(&mut *seen.lock().unwrap());
            let output = String::from_utf8_lossy(&run.outcome.stdout).into_owned();
            assert_eq!(output, theirs, "{arguments:?}, seed {seed}: {seen:?}");
            assert_eq!(seen.taken.len(), 1, "{arguments:?}, seed {seed}: {seen:?}");
            seen
        })
        .collect()
}

#[test]
fn a_task_away_when_its_main_thread_runs_a_new_program_ends_and_is_handed_back() {
    for seen in as_the_reference(Take::Other, &["exec-main"]) {
        assert_eq!(seen.handed_back, 1, "{seen:?}");
        let freed = seen.freed_before_back.as_ref().expect("put back");
        assert!(freed.contains(&seen.taken[0]), "{seen:?}");
    }
}

#[test]
fn a_main_thread_away_when_another_thread_runs_a_new_program_ends_and_the_program_ends_with_its_thread()
 {
    for seen in as_the_reference(Take::Caller, &["exec-worker"]) {
        assert_eq!(seen.handed_back, 1, "{seen:?}");
        // The number goes on with the thread that ran the program.
        let freed = seen.freed_before_back.as_ref().expect("put back");
        assert!(!freed.contains(&seen.taken[0]), "{seen:?}");
    }
}

#[test]
fn the_number_of_a_task_away_is_freed_when_its_process_ends() {
    for seen in as_the_reference(Take::Other, &["exit"]) {
        assert_eq!(seen.handed_back, 1, "{seen:?}");
        let freed = seen.freed_before_back.as_ref().expect("put back");
        assert!(freed.contains(&seen.taken[0]), "{seen:?}");
    }
}

#[test]
fn proc_shows_a_task_away_as_a_task_of_its_process() {
    as_the_reference(Take::Other, &["proc"]);
    as_the_reference(Take::Caller, &["leader-proc"]);
}

#[test]
fn a_robust_lock_held_by_a_task_away_is_marked_as_its_owners_death_when_its_process_ends() {
    as_the_reference(Take::Other, &["robust"]);
}

#[test]
fn a_vfork_parent_goes_on_when_its_child_away_is_killed() {
    as_the_reference(Take::Caller, &["vfork"]);
}

/// At the first mark, copies the caller's process context and takes the whole process (the
/// original, held as if it went on elsewhere); once the kernel is idle, puts the copy on the kernel
/// and the caller back, the other thread staying with the original.
#[derive(Default)]
struct CopyEnds {
    caller: Option<TaskId>,
    log: Arc<Mutex<Vec<String>>>,
}

impl Hooks for CopyEnds {
    fn intercept_system_call_request(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        if request.number != u64::from(general::__NR_getppid) || self.caller.is_some() {
            return ControlFlow::Continue(request);
        }
        self.caller = Some(thread);
        let copy = process.copy_process_context();
        let (original, tasks) = process.suspend_process();
        let mut log = self.log.lock().unwrap();
        log.push(format!("{:?} {:?}", copy.origin(), original.origin()));
        let tasks: Vec<SuspendedTask> = tasks.map(|(_, task)| task).collect();
        log.push(format!("taken {}", tasks.len()));
        TAKEN.with_borrow_mut(|held| held.extend(tasks));
        COPY.set(Some(copy));
        ORIGINAL.set(Some(original));
        ControlFlow::Continue(request)
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let Some(copy) = COPY.take() else {
            return false;
        };
        let mut log = self.log.lock().unwrap();
        let refused = kernel.resume_process(copy, ExecutionConfig::default());
        log.push(format!("copy refused {}", refused.is_some()));
        let caller = self.caller.expect("the caller");
        let (back, staying): (Vec<SuspendedTask>, Vec<SuspendedTask>) = TAKEN
            .take()
            .into_iter()
            .partition(|task| task.id() == caller);
        TAKEN.set(staying);
        log.push(format!(
            "handed back {}",
            kernel.resume_tasks(cpu, back).count()
        ));
        true
    }
}

#[test]
fn a_copys_end_makes_no_last_stores_for_the_tasks_going_on_with_the_original() {
    // On the reference the worker ends with its process, and its lock is marked.
    let theirs = theirs(&["robust-copy"]);
    assert_eq!(theirs, "lock owner died\n");
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let hooks = CopyEnds::default();
        let log = hooks.log.clone();
        let run = ours(&["robust-copy"], Box::new(hooks), seed);
        TAKEN.take();
        ORIGINAL.take();
        let log = log.lock().unwrap().clone();
        assert_eq!(
            log,
            [
                "Copied Suspended",
                "taken 2",
                "copy refused false",
                "handed back 0"
            ],
            "seed {seed}"
        );
        // Here the worker goes on with the original: its lock stays held.
        let output = String::from_utf8_lossy(&run.outcome.stdout).into_owned();
        assert_eq!(output, "lock held\n", "seed {seed}: {log:?}");
    }
}
