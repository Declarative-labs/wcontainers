//! Tasks and process contexts taken off the kernel while the rest of the container goes on: what the
//! process sees must be what Linux shows for tasks that ran throughout (guest/away.c).

use std::cell::RefCell;
use std::ops::{ControlFlow, Range};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wcontainers_abi::{Cpu, SystemCallRequest};
use wcontainers_kernel::linux::general;
use wcontainers_kernel::{
    ContextOrigin, ExecutionConfig, Kernel, MemoryObject, Process, ProcessContext,
    SignalInformation, SuspendedTask, SystemCallResponse, TaskId, reason,
};
use wcontainers_kernel_test::driver::{self, Run};
use wcontainers_kernel_test::guest_directory_entry;
use wcontainers_kernel_test::hooks::Hooks;
use wcontainers_kernel_test::reference::{self, ROOT_DIRECTORY, in_guest_directory};

const PROGRAM: &str = "away";

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

fn counted(run: &Run, which: &str) -> u64 {
    run.counters
        .iter()
        .filter(|counter| counter.reason == which)
        .map(|counter| counter.count)
        .sum()
}

thread_local! {
    /// Tasks and a context the test holds while they are away (they are the kernel's own values).
    static TAKEN: RefCell<Vec<SuspendedTask>> = const { RefCell::new(Vec::new()) };
    static CONTEXT: RefCell<Option<ProcessContext>> = const { RefCell::new(None) };
    static SECOND: RefCell<Option<Kernel>> = const { RefCell::new(None) };
}

/// When a plan takes tasks off and puts them back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Plan {
    /// The child's main thread while it joins its worker; back once the worker has exited.
    Join,
    /// The child's thread in its read; back once the parent waits for it.
    Kill,
    /// The timed reader in its read; back after the main thread's mark (`getppid`), before it writes.
    TimerBeforeWrite,
    /// As `TimerBeforeWrite`, back only once the main thread has written.
    TimerAfterWrite,
    /// The child's whole process while it sleeps; back once the parent waits for it.
    Wait,
    /// A copy of the process's context put on a second kernel while it sleeps; nothing taken.
    Copy,
    /// The process asked for from the hook announcing its signal, its own thread in that step.
    Raise,
}

#[derive(Debug, Default)]
struct Seen {
    notes: Vec<String>,
    mapped: Vec<(u64, Range<u64>)>,
    unmapped: Vec<(u64, Range<u64>)>,
    origin: Option<ContextOrigin>,
    handed_back: usize,
}

struct Script {
    plan: Plan,
    stage: u32,
    processes: Vec<TaskId>,
    threads: Vec<(TaskId, TaskId)>,
    /// The calls asked for, by the process's place in `processes` and the call's number.
    asked: Vec<(usize, TaskId, u64)>,
    seen: Arc<Mutex<Seen>>,
}

impl Script {
    fn asked(&self, process: usize, number: u32) -> usize {
        self.asked
            .iter()
            .filter(|(at, _, asked)| *at == process && *asked == u64::from(number))
            .count()
    }

    fn worker_of(&self, process: TaskId) -> Option<TaskId> {
        self.threads
            .iter()
            .find(|(owner, thread)| *owner == process && *thread != process)
            .map(|(_, thread)| *thread)
    }

    fn take(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu, tasks: &[TaskId]) {
        let taken: Vec<SuspendedTask> = kernel.suspend_tasks(cpu, tasks).map(|(_, t)| t).collect();
        self.seen
            .lock()
            .unwrap()
            .notes
            .push(format!("taken {}", taken.len()));
        TAKEN.with_borrow_mut(|held| held.extend(taken));
    }

    fn put_back(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) {
        let back = kernel.resume_tasks(cpu, TAKEN.take()).count();
        self.seen.lock().unwrap().handed_back += back;
    }
}

impl Hooks for Script {
    fn on_process_starting(&mut self, process: Process<'_>) {
        self.processes.push(process.id());
    }

    fn on_thread_starting(&mut self, process: Process<'_>, thread: TaskId) {
        self.threads.push((process.id(), thread));
    }

    fn intercept_system_call_request(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        let at = self
            .processes
            .iter()
            .position(|known| *known == process.id())
            .unwrap_or(usize::MAX);
        self.asked.push((at, thread, request.number));
        ControlFlow::Continue(request)
    }

    fn on_shared_memory_mapping(
        &mut self,
        _: Process<'_>,
        _: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        self.seen.lock().unwrap().mapped.push((offset, range));
    }

    fn on_shared_memory_unmapped(
        &mut self,
        _: Process<'_>,
        _: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        self.seen.lock().unwrap().unmapped.push((offset, range));
    }

    fn on_signal_delivering(&mut self, process: Process<'_>, _: TaskId, _: &SignalInformation) {
        if self.plan == Plan::Raise && self.stage == 0 {
            self.stage = 1;
            let (context, tasks) = process.suspend_process();
            let mut seen = self.seen.lock().unwrap();
            seen.origin = Some(context.origin());
            seen.notes.push(format!("taken {}", tasks.count()));
        }
    }

    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        const PARENT: usize = 0;
        const CHILD: usize = 1;
        let parent = self.processes.first().copied();
        let child = self.processes.get(CHILD).copied();
        match (self.plan, self.stage) {
            (Plan::Join, 0) if self.asked(CHILD, general::__NR_futex) > 0 => {
                self.take(kernel, cpu, &[child.expect("the child")]);
            }
            (Plan::Join, 1)
                if self.asked.iter().any(|(at, thread, number)| {
                    *at == CHILD
                        && Some(*thread) != child
                        && *number == u64::from(general::__NR_exit)
                }) =>
            {
                self.put_back(kernel, cpu);
            }
            (Plan::Kill, 0) if self.asked(CHILD, general::__NR_read) > 0 => {
                self.take(kernel, cpu, &[child.expect("the child")]);
            }
            (Plan::Kill, 1) if self.asked(PARENT, general::__NR_wait4) > 0 => {
                self.put_back(kernel, cpu);
            }
            (Plan::TimerBeforeWrite | Plan::TimerAfterWrite, 0) => {
                let parent = parent.expect("the process");
                let Some(reader) = self.worker_of(parent) else {
                    return false;
                };
                let reading = self.asked.iter().any(|(_, thread, number)| {
                    *thread == reader && *number == u64::from(general::__NR_read)
                });
                if !reading {
                    return false;
                }
                self.take(kernel, cpu, &[reader]);
            }
            (Plan::TimerBeforeWrite, 1) if self.asked(PARENT, general::__NR_getppid) > 0 => {
                self.put_back(kernel, cpu);
            }
            (Plan::TimerAfterWrite, 1) if self.asked(PARENT, general::__NR_write) > 0 => {
                self.put_back(kernel, cpu);
            }
            (Plan::Wait, 0) if self.asked(CHILD, general::__NR_nanosleep) > 0 => {
                let (context, tasks) = kernel
                    .suspend_process(cpu, child.expect("the child"))
                    .expect("the child's process");
                self.seen.lock().unwrap().origin = Some(context.origin());
                CONTEXT.set(Some(context));
                TAKEN.with_borrow_mut(|held| held.extend(tasks.map(|(_, task)| task)));
            }
            // The parent's second wait is the one that waits.
            (Plan::Wait, 1) if self.asked(PARENT, general::__NR_wait4) > 1 => {
                let context = CONTEXT.take().expect("the context");
                let back = kernel.resume_process(context, ExecutionConfig::default());
                assert!(back.is_none(), "the context is put back");
                self.put_back(kernel, cpu);
            }
            (Plan::Copy, 0) if self.asked(PARENT, general::__NR_nanosleep) > 0 => {
                let process = parent.expect("the process");
                let copy = kernel.copy_process_context(process).expect("a copy");
                self.seen.lock().unwrap().origin = Some(copy.origin());
                let mut second = Kernel::new();
                let back = second.resume_process(copy, ExecutionConfig::default());
                assert!(back.is_none(), "the copy is put on the second kernel");
                assert!(
                    kernel.copy_process_context(process).is_some(),
                    "the process stays"
                );
                SECOND.set(Some(second));
            }
            _ => return false,
        }
        self.stage += 1;
        true
    }
}

fn run_plan(plan: Plan, arguments: &[&str], seed: u64) -> (Run, Seen) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let script = Script {
        plan,
        stage: 0,
        processes: Vec::new(),
        threads: Vec::new(),
        asked: Vec::new(),
        seen: seen.clone(),
    };
    let run = ours(arguments, Box::new(script), seed);
    TAKEN.take();
    CONTEXT.take();
    SECOND.take();
    let seen = std::mem::take(&mut *seen.lock().unwrap());
    (run, seen)
}

/// Runs `plan` under every seed and asks that what the program printed is the reference's.
fn as_the_reference(plan: Plan, arguments: &[&str]) -> Vec<Seen> {
    let theirs = theirs(arguments);
    wcontainers_kernel_test::schedule_seeds()
        .map(|seed| {
            let (run, seen) = run_plan(plan, arguments, seed);
            let label = format!("{plan:?} {arguments:?}, seed {seed}: {seen:?}");
            assert_eq!(stdout(&run), theirs, "{label} {:?}", run.counters);
            assert!(
                seen.notes.iter().any(|note| note.starts_with("taken")) || seen.origin.is_some(),
                "{label}: the plan acted"
            );
            seen
        })
        .collect()
}

#[test]
fn a_process_whose_main_thread_is_away_goes_on_when_its_other_thread_ends() {
    for seen in as_the_reference(Plan::Join, &["join"]) {
        assert_eq!(seen.handed_back, 0, "{seen:?}");
    }
}

#[test]
fn a_signal_to_a_process_whose_tasks_are_away_waits_for_them_and_sigkill_does_not() {
    let terminate = general::SIGTERM.to_string();
    for seen in as_the_reference(Plan::Kill, &["kill", &terminate]) {
        assert_eq!(seen.handed_back, 0, "{seen:?}");
    }
    // SIGKILL ends the process at once: the parent reaps it without its task coming back.
    let kill = general::SIGKILL.to_string();
    as_the_reference(Plan::Kill, &["kill", &kill]);
}

#[test]
fn a_threads_timer_firing_while_it_is_away_interrupts_its_read_when_it_is_back() {
    as_the_reference(Plan::TimerBeforeWrite, &["timer"]);
}

/// Linux interrupted the read when the stop came; here the call runs again when the task is put
/// back, and finds the byte written meanwhile before it looks at the signal.
#[test]
#[ignore = "known failure: a read whose wait was met while it was away answers the data, not EINTR"]
fn a_threads_timer_firing_while_it_is_away_interrupts_its_read_though_the_byte_came() {
    as_the_reference(Plan::TimerAfterWrite, &["timer"]);
}

#[test]
fn a_parent_waits_for_a_child_whose_process_is_away() {
    for seen in as_the_reference(Plan::Wait, &["wait"]) {
        assert_eq!(seen.origin, Some(ContextOrigin::Suspended), "{seen:?}");
        assert_eq!(seen.handed_back, 0, "{seen:?}");
    }
}

#[test]
fn shared_memory_is_announced_on_the_first_kernel_after_a_copy_goes_on_a_second() {
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let (run, seen) = run_plan(Plan::Copy, &["mapping"], seed);
        let output = stdout(&run);
        let label = format!("seed {seed}: {output:?} {seen:?}");
        assert_eq!(seen.origin, Some(ContextOrigin::Copied), "{label}");
        let line = output
            .lines()
            .find_map(|line| line.strip_prefix("mapped "))
            .expect("the program mapped its file");
        let bounds: Vec<u64> = line
            .split(' ')
            .map(|number| u64::from_str_radix(number, 16).expect("an address"))
            .collect();
        let range = bounds[0]..bounds[1];
        assert_eq!(seen.mapped, vec![(0, range.clone())], "{label}");
        assert_eq!(seen.unmapped, vec![(0, range)], "{label}");
        assert!(output.ends_with("unmapped\n"), "{label}");
    }
}

#[test]
fn a_hooks_process_whose_thread_is_in_its_step_stays_and_says_its_context_is_a_copy() {
    let theirs = theirs(&["raise"]);
    for seed in wcontainers_kernel_test::schedule_seeds() {
        let (run, seen) = run_plan(Plan::Raise, &["raise"], seed);
        let label = format!("seed {seed}: {seen:?}");
        assert_eq!(stdout(&run), theirs, "{label}");
        assert_eq!(seen.origin, Some(ContextOrigin::Copied), "{label}");
        assert_eq!(seen.notes, vec!["taken 0".to_owned()], "{label}");
        assert_eq!(
            counted(&run, reason::TASK_IN_A_STEP_NOT_SUSPENDED),
            1,
            "{label} {:?}",
            run.counters
        );
    }
}
