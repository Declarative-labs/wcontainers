//! Tasks and process contexts taken off the kernel while the rest of the container goes on: what the
//! process sees must be what Linux shows for tasks that ran throughout (guest/away.c).

use std::cell::RefCell;
use std::ops::{ControlFlow, Range};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use std::collections::BTreeMap;

use wcontainers_abi::{Cpu, PAGE_SIZE, SystemCallRequest};
use wcontainers_executor_unicorn::driver::{self, Run};
use wcontainers_executor_unicorn::guest_directory_entry;
use wcontainers_executor_unicorn::hooks::Hooks;
use wcontainers_executor_unicorn::reference::{self, ROOT_DIRECTORY, in_guest_directory};
use wcontainers_kernel::UserId;
use wcontainers_kernel::signal::Signal;
use wcontainers_kernel::signal_information::{ProcessIdentity, SendCode};
use wcontainers_kernel_linux::linux::general;
use wcontainers_kernel_linux::{
    ContextOrigin, ExecutionConfig, Kernel, MemoryObject, Process, ProcessContext,
    SignalInformation, SuspendedTask, SystemCallResponse, TaskId, reason,
};

const PROGRAM: &str = "away";

fn out() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(PROGRAM)
}

fn program() -> PathBuf {
    wcontainers_executor_unicorn::guest::build(PROGRAM, &[], &out())
}

fn theirs(arguments: &[&str]) -> String {
    String::from_utf8_lossy(&reference::run(&program(), arguments).stdout).into_owned()
}

fn ours(arguments: &[&str], hooks: Box<dyn Hooks>, seed: u64) -> Run {
    let program = program();
    let mut files = wcontainers_executor_unicorn::image::reference_image().to_vec();
    files.extend(wcontainers_executor_unicorn::image::host_directory(
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
        address_space: wcontainers_executor_unicorn::standin::SPACE_32_BIT,
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
    /// The parent's whole process while it waits; back once its child has asked to exit.
    Leave,
    /// The parent's whole process from the hook announcing its first piece of shared memory; back
    /// once its child has asked to exit.
    Pieces,
    /// The reader in its read; the embedder sends it a signal at the main thread's first mark
    /// (`getppid`), and it is back at the second.
    Sent,
    /// The child in its socket read; back at the parent's mark (`getppid`).
    Socket,
    /// Nothing is taken.
    Nothing,
}

#[derive(Debug, Default)]
struct Seen {
    notes: Vec<String>,
    mapped: Vec<(u64, Range<u64>)>,
    unmapped: Vec<(u64, Range<u64>)>,
    origin: Option<ContextOrigin>,
    handed_back: usize,
    /// Every shared memory announcement: the process, whether it starts, the offset and range.
    pieces: Vec<(TaskId, bool, u64, Range<u64>)>,
    /// How many announcements came before the plan put the process back.
    pieces_before_back: Option<usize>,
    not_sent: u64,
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
        if let Some(context) = CONTEXT.take() {
            let back = kernel.resume_process(context, ExecutionConfig::default());
            assert!(back.is_none(), "the context is put back");
        }
        let back = kernel.resume_tasks(cpu, TAKEN.take()).count();
        let mut seen = self.seen.lock().unwrap();
        seen.handed_back += back;
        seen.pieces_before_back = Some(seen.pieces.len());
    }

    /// Notes a shared memory announcement; in `Pieces`, takes the parent at its first.
    fn piece(&mut self, process: Process<'_>, start: bool, offset: u64, range: Range<u64>) {
        assert!(process.info().memory_mapped() > 0);
        let id = process.id();
        self.seen
            .lock()
            .unwrap()
            .pieces
            .push((id, start, offset, range));
        if self.plan == Plan::Pieces && self.stage == 0 && self.processes.first() == Some(&id) {
            self.stage = 1;
            let (context, tasks) = process.suspend_process();
            let mut seen = self.seen.lock().unwrap();
            seen.origin = Some(context.origin());
            CONTEXT.set(Some(context));
            TAKEN.with_borrow_mut(|held| held.extend(tasks.map(|(_, task)| task)));
            seen.notes.push("taken in its hook".to_owned());
        }
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
        self.asked.push((at, thread, request.number.0));
        ControlFlow::Continue(request)
    }

    fn on_shared_memory_mapping(
        &mut self,
        process: Process<'_>,
        _: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        self.seen
            .lock()
            .unwrap()
            .mapped
            .push((offset, range.clone()));
        self.piece(process, true, offset, range);
    }

    fn on_shared_memory_unmapped(
        &mut self,
        process: Process<'_>,
        _: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        self.seen
            .lock()
            .unwrap()
            .unmapped
            .push((offset, range.clone()));
        self.piece(process, false, offset, range);
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
            (Plan::TimerBeforeWrite | Plan::TimerAfterWrite | Plan::Sent, 0) => {
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
                assert!(CONTEXT.with_borrow(Option::is_some), "the context");
                self.put_back(kernel, cpu);
            }
            (Plan::Leave, 0)
                if self.asked(PARENT, general::__NR_wait4) > 0
                    && self.asked(CHILD, general::__NR_nanosleep) > 0 =>
            {
                let (context, tasks) = kernel
                    .suspend_process(cpu, parent.expect("the parent"))
                    .expect("the parent's process");
                self.seen.lock().unwrap().origin = Some(context.origin());
                CONTEXT.set(Some(context));
                TAKEN.with_borrow_mut(|held| held.extend(tasks.map(|(_, task)| task)));
                self.seen.lock().unwrap().notes.push("taken".to_owned());
            }
            (Plan::Leave | Plan::Pieces, 1) if self.asked(CHILD, general::__NR_exit_group) > 0 => {
                self.put_back(kernel, cpu);
            }
            (Plan::Sent, 1) if self.asked(PARENT, general::__NR_getppid) > 0 => {
                let parent = parent.expect("the process");
                let reader = self.worker_of(parent).expect("the reader");
                kernel
                    .process(cpu, parent)
                    .expect("the process is here")
                    .send_signal(reader, user_signal(Signal::SIGUSR1));
            }
            (Plan::Sent, 2) if self.asked(PARENT, general::__NR_getppid) > 1 => {
                self.put_back(kernel, cpu);
            }
            (Plan::Socket, 0) if self.asked(CHILD, general::__NR_recvfrom) > 0 => {
                self.take(kernel, cpu, &[child.expect("the child")]);
            }
            (Plan::Socket, 1) if self.asked(PARENT, general::__NR_getppid) > 0 => {
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
    as_the_reference_runs(plan, arguments, arguments)
}

/// As [`as_the_reference`], the reference run with `reference` (which does there what the plan
/// does here).
fn as_the_reference_runs(plan: Plan, arguments: &[&str], reference: &[&str]) -> Vec<Seen> {
    let theirs = theirs(reference);
    wcontainers_executor_unicorn::schedule_seeds()
        .map(|seed| {
            let (run, mut seen) = run_plan(plan, arguments, seed);
            seen.not_sent = counted(&run, reason::SIGNAL_NOT_SENT);
            let label = format!("{plan:?} {arguments:?}, seed {seed}: {seen:?}");
            assert_eq!(stdout(&run), theirs, "{label} {:?}", run.counters);
            assert!(
                plan == Plan::Nothing
                    || seen.notes.iter().any(|note| note.starts_with("taken"))
                    || seen.origin.is_some(),
                "{label}: the plan acted"
            );
            seen
        })
        .collect()
}

/// A record of `signal` sent from outside the container (no sender process).
fn user_signal(signal: Signal) -> SignalInformation {
    SignalInformation::SentByProcess {
        signal,
        code: SendCode::SI_USER,
        sender: ProcessIdentity {
            process: TaskId(0),
            real_user: UserId(0),
        },
    }
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
    for seed in wcontainers_executor_unicorn::schedule_seeds() {
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
    for seed in wcontainers_executor_unicorn::schedule_seeds() {
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

#[test]
fn a_parent_away_while_its_forked_child_ends_goes_on_when_it_is_back() {
    for seen in as_the_reference(Plan::Leave, &["leave", "fork"]) {
        assert_eq!(seen.origin, Some(ContextOrigin::Suspended), "{seen:?}");
        assert_eq!(seen.handed_back, 0, "{seen:?}");
    }
}

#[test]
fn a_parent_away_while_a_child_in_its_memory_ends_goes_on_when_it_is_back() {
    for seen in as_the_reference(Plan::Leave, &["leave", "vm"]) {
        assert_eq!(seen.origin, Some(ContextOrigin::Suspended), "{seen:?}");
        assert_eq!(seen.handed_back, 0, "{seen:?}");
    }
}

#[test]
fn a_process_taken_in_its_own_sharing_hook_is_told_the_rest_when_it_is_back() {
    for seen in as_the_reference(Plan::Pieces, &["pieces"]) {
        let label = format!("{seen:?}");
        assert_eq!(seen.origin, Some(ContextOrigin::Suspended), "{label}");
        assert_eq!(seen.handed_back, 0, "{label}");
        // Each end names pages announced before at the same place in the source, and all end.
        let mut told: BTreeMap<(TaskId, u64), u64> = BTreeMap::new();
        for (process, start, offset, range) in &seen.pieces {
            for page in range.clone().step_by(PAGE_SIZE as usize) {
                let place = offset + (page - range.start);
                if *start {
                    let before = told.insert((*process, page), place);
                    assert_eq!(before, None, "{label}: {page:#x} announced twice");
                } else {
                    let before = told.remove(&(*process, page));
                    assert_eq!(before, Some(place), "{label}: {page:#x} ended");
                }
            }
        }
        assert!(told.is_empty(), "{label}: {told:?}");
        // The parent was told of both its pieces, the second once it was back.
        let parent = seen.pieces.first().expect("an announcement").0;
        let starts: Vec<usize> = (0..seen.pieces.len())
            .filter(|&at| seen.pieces[at].0 == parent && seen.pieces[at].1)
            .collect();
        assert_eq!(starts.len(), 2, "{label}");
        let back = seen.pieces_before_back.expect("put back");
        assert!(starts[1] >= back, "{label}");
    }
}

#[test]
fn a_signal_the_embedder_sends_to_a_task_away_waits_for_it() {
    for seen in as_the_reference_runs(Plan::Sent, &["sent", "embedder"], &["sent", "self"]) {
        assert_eq!(seen.not_sent, 0, "{seen:?}");
        assert_eq!(seen.handed_back, 0, "{seen:?}");
    }
}

#[test]
fn a_socket_wait_with_a_receive_timeout_taken_off_answers_as_a_stop_makes_it_answer() {
    as_the_reference_runs(
        Plan::Socket,
        &["socket", "embedder"],
        &["socket", "stopping"],
    );
}

#[test]
fn a_socket_wait_with_a_receive_timeout_is_not_restarted_after_a_handler() {
    as_the_reference(Plan::Nothing, &["socket", "handler"]);
}
