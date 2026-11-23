//! A stop landing at every instruction a stub thread runs in a turn: the test traces the thread (ptrace, the test's
//! tool alone), steps it one instruction at a time from its wait, and in each run sends the stop at the next
//! instruction of the path, from the wait's return through the commands, a `clone` among them, the resume, the
//! publish and the wake to the next wait, and through a thread's end.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use rustix::mm::ProtFlags;
use wcontainers_executor_host_process::layout::GUEST_START;
use wcontainers_executor_host_process::pages::{Command, Resume};
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::stub::{StopsLanded, Stub, TurnEnd};
use wcontainers_executor_host_process::zygote::Zygote;

/// One embedder process has one zygote place, so tests running at once start theirs one at a time.
static STARTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn zygote() -> Zygote {
    let probe = HostProbe::new().expect("the host is refused");
    let _one_at_a_time = STARTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Zygote::start(&probe).expect("the zygote did not start")
}

fn knob(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

const PAGE: usize = 4096;

/// How long a step may take before the thread is taken to be blocked in a host call.
const BLOCKED: Duration = Duration::from_millis(500);

/// How long a turn may take to end once its thread runs untraced again.
const TURN_LIMIT: Duration = Duration::from_secs(2);

/// More steps than any path of a turn takes.
const MOST_STEPS: usize = 100_000;

/// How often a pc is seen before the path is taken to be in a loop that only a stop ends.
const LOOPED: usize = 50;

/// The stop the test sends by ptrace, the same signal the executor sends.
const STOP: Signal = Signal::SIGUSR1;
const _: () = assert!(
    STOP as u32 == wcontainers_executor_host_process::stub::STOP_SIGNAL,
    "the test's stop is the executor's"
);

fn map_page() -> Command {
    Command::Map {
        address: GUEST_START,
        length: PAGE,
        permissions: ProtFlags::READ | ProtFlags::WRITE,
        offset: GUEST_START,
    }
}

fn protect_page() -> Command {
    Command::Protect {
        address: GUEST_START,
        length: PAGE,
        permissions: ProtFlags::READ,
    }
}

fn unmap_page() -> Command {
    Command::Unmap {
        address: GUEST_START,
        length: PAGE,
    }
}

/// The permissions /proc shows for the mapping holding `address` (the test may read /proc; the executor never does).
fn permissions_at(stub: &Stub, address: usize) -> Option<String> {
    let maps = std::fs::read_to_string(format!("/proc/{}/maps", stub.process().as_raw_nonzero()))
        .expect("maps");
    maps.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let (start, end) = fields.next()?.split_once('-')?;
        let start = usize::from_str_radix(start, 16).ok()?;
        let end = usize::from_str_radix(end, 16).ok()?;
        (start <= address && address < end).then(|| fields.next().map(str::to_owned))?
    })
}

/// The host threads /proc lists for the stub, waiting up to 1 s for it to reach `expected` (the host lists a thread
/// until it reaps it, a moment after its exit-clear word is cleared).
fn host_threads(stub: &Stub, expected: usize) -> usize {
    let until = Instant::now() + Duration::from_secs(1);
    loop {
        let count = std::fs::read_dir(format!("/proc/{}/task", stub.process().as_raw_nonzero()))
            .expect("tasks")
            .count();
        if count == expected || Instant::now() >= until {
            return count;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Waits until a stub thread is blocked in its wait for a turn, as /proc's line for its host call shows (a thread
/// that just published, or was just made, is not there yet).
fn until_waiting(stub: &Stub, thread: usize) {
    let path = format!(
        "/proc/{}/task/{}/syscall",
        stub.process().as_raw_nonzero(),
        stub.reported_thread_id(thread)
    );
    let until = Instant::now() + Duration::from_secs(1);
    loop {
        let line = std::fs::read_to_string(&path).expect("syscall");
        let fields: Vec<&str> = line.split_whitespace().collect();
        let call = fields.first().and_then(|call| call.parse::<u32>().ok());
        let operation = fields
            .get(2)
            .and_then(|operation| i64::from_str_radix(operation.trim_start_matches("0x"), 16).ok());
        if call == Some(linux_raw_sys::general::__NR_futex)
            && operation.is_some_and(|operation| {
                operation as i32 & linux_raw_sys::general::FUTEX_CMD_MASK
                    == linux_raw_sys::general::FUTEX_WAIT as i32
            })
        {
            return;
        }
        assert!(
            Instant::now() < until,
            "thread {thread} never waited: {line}"
        );
        std::thread::yield_now();
    }
}

/// Where a thread's stops landed once it is back in its wait: a turn's end is published before a stop that landed
/// after it has been counted.
fn settled(stub: &Stub, thread: usize) -> StopsLanded {
    until_waiting(stub, thread);
    stub.stops_landed(thread)
}

fn total(landed: StopsLanded) -> u64 {
    landed.in_turn + landed.in_publish + landed.in_wait + landed.in_end + landed.in_call
}

#[cfg(target_arch = "aarch64")]
fn pc(thread: Pid) -> u64 {
    ptrace::getregs(thread).expect("registers").pc
}

#[cfg(target_arch = "x86_64")]
fn pc(thread: Pid) -> u64 {
    ptrace::getregs(thread).expect("registers").rip
}

/// The host call's number and its second argument, as a host call at the thread's pc would take them.
#[cfg(target_arch = "aarch64")]
fn call_and_operation(registers: &libc::user_regs_struct) -> (u64, u64) {
    (registers.regs[8], registers.regs[1])
}

#[cfg(target_arch = "x86_64")]
fn call_and_operation(registers: &libc::user_regs_struct) -> (u64, u64) {
    (registers.rax, registers.rsi)
}

/// The bytes of the instructions at `at` in a traced thread, read a word at a time.
fn code(thread: Pid, at: u64, bytes: usize) -> Vec<u8> {
    (0..bytes.div_ceil(size_of::<u64>()))
        .flat_map(|word| {
            let address = at + (word * size_of::<u64>()) as u64;
            ptrace::read(thread, address as ptrace::AddressType)
                .expect("the code at the pc")
                .to_le_bytes()
        })
        .collect()
}

#[cfg(target_arch = "aarch64")]
fn is_host_call(thread: Pid, at: u64) -> bool {
    let word = code(thread, at, size_of::<u32>());
    let word = u32::from_le_bytes(word[..size_of::<u32>()].try_into().expect("a word"));
    wcontainers_arm64::decode(word) == wcontainers_arm64::Instruction::Syscall
}

#[cfg(target_arch = "x86_64")]
fn is_host_call(thread: Pid, at: u64) -> bool {
    // One word holds every instruction a host call can be; a longer one decodes as invalid here, so not a call.
    let bytes = code(thread, at, size_of::<u64>());
    let mut decoder =
        iced_x86::Decoder::with_ip(u64::BITS, &bytes, at, iced_x86::DecoderOptions::NONE);
    decoder.decode().mnemonic() == iced_x86::Mnemonic::Syscall
}

/// One stub thread traced by the test.
struct Traced {
    thread: Pid,
}

/// How one step ended.
#[derive(Debug, PartialEq, Eq)]
enum Stepped {
    /// The thread ran one instruction and is stopped at the next.
    At(u64),
    /// The thread is in a host call that waits, and stopped there by the test.
    Blocked,
    /// The thread ended.
    Ended,
}

impl Traced {
    /// Traces a stub thread, which is waiting for a turn, and stops it there.
    fn attach(stub: &Stub, thread: usize) -> Self {
        until_waiting(stub, thread);
        let thread = Pid::from_raw(stub.reported_thread_id(thread) as i32);
        ptrace::seize(thread, ptrace::Options::empty()).expect("seize");
        ptrace::interrupt(thread).expect("interrupt");
        match waitpid(thread, Some(WaitPidFlag::__WALL)).expect("wait for the stop") {
            WaitStatus::PtraceEvent(_, _, event) if event == libc::PTRACE_EVENT_STOP => {}
            other => panic!("the traced thread stopped as {other:?}"),
        }
        Self { thread }
    }

    /// Runs one instruction, taking the thread to be blocked when it has not stopped within `limit`.
    fn step(&self, limit: Duration) -> Stepped {
        ptrace::step(self.thread, None).expect("step");
        let until = Instant::now() + limit;
        loop {
            match waitpid(
                self.thread,
                Some(WaitPidFlag::__WALL | WaitPidFlag::WNOHANG),
            )
            .expect("wait for the step")
            {
                WaitStatus::StillAlive => {}
                WaitStatus::Stopped(_, Signal::SIGTRAP) => return Stepped::At(pc(self.thread)),
                WaitStatus::Exited(..) => return Stepped::Ended,
                other => panic!("a step ended as {other:?}"),
            }
            if Instant::now() >= until {
                ptrace::interrupt(self.thread).expect("interrupt");
                match waitpid(self.thread, Some(WaitPidFlag::__WALL)).expect("wait for the stop") {
                    WaitStatus::PtraceEvent(_, _, event) if event == libc::PTRACE_EVENT_STOP => {
                        return Stepped::Blocked;
                    }
                    WaitStatus::Stopped(_, Signal::SIGTRAP) => return Stepped::At(pc(self.thread)),
                    other => panic!("a blocked step ended as {other:?}"),
                }
            }
            std::thread::sleep(Duration::from_micros(20));
        }
    }

    /// Whether the thread's next instruction is a host call that waits on a futex word. A thread stepped into a
    /// call that blocks and let go there takes a step trap when the call returns (seen on ARM64's Linux 7.0), so
    /// the path stops before it.
    fn about_to_wait(&self) -> bool {
        let registers = ptrace::getregs(self.thread).expect("registers");
        let (call, operation) = call_and_operation(&registers);
        call == u64::from(linux_raw_sys::general::__NR_futex)
            && operation as i32 & linux_raw_sys::general::FUTEX_CMD_MASK
                == linux_raw_sys::general::FUTEX_WAIT as i32
            && is_host_call(self.thread, pc(self.thread))
    }

    /// Lets the thread go on untraced, delivering `signal` at the instruction it is stopped at.
    fn detach(self, signal: Option<Signal>) {
        ptrace::detach(self.thread, signal).expect("detach");
    }
}

/// What a scenario's turn does, and what each of its ends must show.
trait Scenario {
    /// Puts the stub back to the state before the turn, untraced.
    fn reset(&mut self, stub: &mut Stub);
    /// The thread that takes the turn.
    fn thread(&self) -> usize;
    /// Gives the turn without waiting for it; its number.
    fn begin(&mut self, stub: &mut Stub) -> u32;
    /// Whether the path is complete after this step, which reached a pc `seen` times and whose next instruction
    /// `waits` on a futex word: the dry run stops there.
    fn path_ends(
        &self,
        stub: &Stub,
        turn: u32,
        stepped: &Stepped,
        seen: usize,
        waits: bool,
    ) -> bool;
    /// Checks how the turn ended after a stop at step `step`, and puts right what it made; the name of where the
    /// stop landed.
    fn check(
        &mut self,
        stub: &mut Stub,
        turn: u32,
        step: usize,
        before: StopsLanded,
    ) -> &'static str;
}

/// Where a stop landed, by the counter it raised.
fn landed(before: StopsLanded, after: StopsLanded) -> &'static str {
    assert_eq!(
        total(after),
        total(before) + 1,
        "one stop sent, {after:?} after {before:?}"
    );
    if after.in_turn > before.in_turn {
        "turn"
    } else if after.in_publish > before.in_publish {
        "publish"
    } else if after.in_wait > before.in_wait {
        "wait"
    } else if after.in_end > before.in_end {
        "end"
    } else {
        "call"
    }
}

/// Runs a scenario's turn once traced to find its path, then once for every `stride`-th instruction of it with the
/// stop sent there; how many stops landed in each window.
fn every_instruction(
    scenario: &mut impl Scenario,
    stub: &mut Stub,
) -> BTreeMap<&'static str, usize> {
    scenario.reset(stub);
    let traced = Traced::attach(stub, scenario.thread());
    let turn = scenario.begin(stub);
    let mut path = Vec::new();
    let mut seen: BTreeMap<u64, usize> = BTreeMap::new();
    loop {
        assert!(
            path.len() < MOST_STEPS,
            "the path runs past {MOST_STEPS} steps"
        );
        let stepped = traced.step(BLOCKED);
        let times = match stepped {
            Stepped::At(at) => {
                path.push(at);
                let times = seen.entry(at).or_default();
                *times += 1;
                *times
            }
            Stepped::Blocked | Stepped::Ended => 0,
        };
        let waits = matches!(stepped, Stepped::At(_)) && traced.about_to_wait();
        if scenario.path_ends(stub, turn, &stepped, times, waits) {
            if stepped != Stepped::Ended {
                traced.detach(None);
            }
            break;
        }
        assert!(
            matches!(stepped, Stepped::At(_)),
            "the path ended early, {stepped:?} after {} steps",
            path.len()
        );
    }
    let before = stub.stops_landed(scenario.thread());
    scenario.check(stub, turn, 0, before);
    let stride = knob("WCHP_STEP_STRIDE", 1).max(1);
    let mut windows = BTreeMap::new();
    for step in (1..=path.len()).step_by(stride) {
        scenario.reset(stub);
        let thread = scenario.thread();
        let traced = Traced::attach(stub, thread);
        let turn = scenario.begin(stub);
        for (taken, expected) in path[..step].iter().enumerate() {
            assert_eq!(
                traced.step(TURN_LIMIT),
                Stepped::At(*expected),
                "step {} of the path to {step}",
                taken + 1
            );
        }
        let before = stub.stops_landed(thread);
        traced.detach(Some(STOP));
        *windows
            .entry(scenario.check(stub, turn, step, before))
            .or_default() += 1;
    }
    eprintln!(
        "{} steps, every {stride}th stopped: {windows:?}",
        path.len()
    );
    windows
}

fn wait(stub: &Stub, thread: usize, turn: u32, step: usize) -> TurnEnd {
    match stub.wait_within(thread, turn, TURN_LIMIT).expect("wait") {
        Some(TurnEnd::StubEnded) => panic!("the stub ended after a stop at step {step}"),
        Some(end) => end,
        None => panic!("the turn did not end within {TURN_LIMIT:?} of a stop at step {step}"),
    }
}

/// A turn of commands, a thread's making among them, that reports back.
struct Commands {
    /// The command that makes the thread, holding its index.
    made: Option<Command>,
}

impl Commands {
    fn commands(&self) -> [Command; 3] {
        [map_page(), protect_page(), self.made.expect("begun")]
    }
}

impl Scenario for Commands {
    fn reset(&mut self, stub: &mut Stub) {
        assert_eq!(
            stub.turn(0, &[unmap_page()]),
            Ok(TurnEnd::Done {
                commands_applied: 1
            })
        );
        assert_eq!(permissions_at(stub, GUEST_START), None);
        assert_eq!(host_threads(stub, 1), 1);
    }

    fn thread(&self) -> usize {
        0
    }

    fn begin(&mut self, stub: &mut Stub) -> u32 {
        self.made = Some(stub.new_thread().expect("a free index"));
        let commands = self.commands();
        stub.begin(0, &commands, Resume::Ping).expect("begin")
    }

    fn path_ends(&self, stub: &Stub, turn: u32, _: &Stepped, _: usize, waits: bool) -> bool {
        waits
            && stub
                .wait_within(0, turn, Duration::ZERO)
                .expect("look")
                .is_some()
    }

    fn check(
        &mut self,
        stub: &mut Stub,
        turn: u32,
        step: usize,
        before: StopsLanded,
    ) -> &'static str {
        let all = self.commands().len() as u64;
        let applied = match wait(stub, 0, turn, step) {
            TurnEnd::Done { commands_applied } if commands_applied == all => all,
            TurnEnd::Stopped { commands_applied }
                if commands_applied == all || commands_applied == 0 =>
            {
                commands_applied
            }
            other => panic!("after a stop at step {step} the turn ended {other:?}"),
        };
        let Some(Command::NewThread { thread: made, .. }) = self.made.take() else {
            unreachable!("begun")
        };
        // The commands took effect all or none, the thread made exactly once.
        let expected = if applied == all {
            Some("r--s".to_owned())
        } else {
            None
        };
        assert_eq!(permissions_at(stub, GUEST_START), expected, "step {step}");
        assert_eq!(
            host_threads(stub, if applied == all { 2 } else { 1 }),
            if applied == all { 2 } else { 1 },
            "step {step}"
        );
        let where_ = if step == 0 {
            ""
        } else {
            landed(before, settled(stub, 0))
        };
        if applied == all {
            assert_ne!(stub.reported_thread_id(made), 0, "step {step}");
            assert_eq!(
                stub.turn(made, &[]),
                Ok(TurnEnd::Done {
                    commands_applied: 0
                }),
                "the made thread answers after step {step}"
            );
            stub.end_thread(made).expect("end");
            assert!(
                stub.thread_gone(made, Duration::from_secs(1))
                    .expect("gone"),
                "step {step}"
            );
        } else {
            stub.release_thread(made);
        }
        assert_eq!(
            stub.turn(0, &[]),
            Ok(TurnEnd::Done {
                commands_applied: 0
            }),
            "the thread answers after step {step}"
        );
        where_
    }
}

/// A turn of commands that then runs until a stop ends it.
struct Spin;

impl Scenario for Spin {
    fn reset(&mut self, stub: &mut Stub) {
        assert_eq!(
            stub.turn(0, &[unmap_page()]),
            Ok(TurnEnd::Done {
                commands_applied: 1
            })
        );
    }

    fn thread(&self) -> usize {
        0
    }

    fn begin(&mut self, stub: &mut Stub) -> u32 {
        stub.begin(0, &[map_page(), protect_page()], Resume::Spin)
            .expect("begin")
    }

    fn path_ends(&self, _: &Stub, _: u32, _: &Stepped, seen: usize, _: bool) -> bool {
        seen >= LOOPED
    }

    fn check(
        &mut self,
        stub: &mut Stub,
        turn: u32,
        step: usize,
        before: StopsLanded,
    ) -> &'static str {
        let mut end = stub.wait_within(0, turn, SPIN_LIMIT).expect("wait");
        let mut where_ = "";
        if end.is_none() {
            // A stop in the wait, before the turn is taken, changes nothing: the runner sends stops until one ends
            // the spin, as here. The dry run left the thread spinning untraced.
            if step != 0 {
                where_ = landed(before, stub.stops_landed(0));
                assert_eq!(where_, "wait", "a stop at step {step} lost");
            }
            end = Some(stop_spin(stub, turn, step));
        }
        let applied = match end.expect("ended") {
            TurnEnd::Stopped { commands_applied }
                if commands_applied == 2 || commands_applied == 0 =>
            {
                commands_applied
            }
            other => panic!("after a stop at step {step} the spin ended {other:?}"),
        };
        let expected = (applied == 2).then(|| "r--s".to_owned());
        assert_eq!(permissions_at(stub, GUEST_START), expected, "step {step}");
        if step != 0 && where_.is_empty() {
            where_ = landed(before, settled(stub, 0));
        }
        assert_eq!(
            stub.turn(0, &[]),
            Ok(TurnEnd::Done {
                commands_applied: 0
            }),
            "the thread answers after step {step}"
        );
        where_
    }
}

/// How long a spin may run after its stop before that stop is taken to have landed before the turn was taken.
const SPIN_LIMIT: Duration = Duration::from_millis(200);

/// Sends the stop each millisecond until the spinning turn ends; its end.
fn stop_spin(stub: &Stub, turn: u32, step: usize) -> TurnEnd {
    for _ in 0..10_000 {
        stub.stop(0).expect("stop");
        if let Some(end) = stub
            .wait_within(0, turn, Duration::from_millis(1))
            .expect("wait")
        {
            return end;
        }
    }
    panic!("the spin ignored 10,000 stops after step {step}");
}

/// A thread told to end.
struct End {
    thread: Option<usize>,
}

impl Scenario for End {
    fn reset(&mut self, stub: &mut Stub) {
        let command = stub.new_thread().expect("a free index");
        let Command::NewThread { thread, .. } = command else {
            unreachable!("a new thread's command")
        };
        assert_eq!(
            stub.turn(0, &[command]),
            Ok(TurnEnd::Done {
                commands_applied: 1
            })
        );
        self.thread = Some(thread);
    }

    fn thread(&self) -> usize {
        self.thread.expect("made")
    }

    fn begin(&mut self, stub: &mut Stub) -> u32 {
        stub.begin(self.thread(), &[], Resume::ExitThread)
            .expect("begin")
    }

    fn path_ends(&self, _: &Stub, _: u32, stepped: &Stepped, _: usize, _: bool) -> bool {
        *stepped == Stepped::Ended
    }

    fn check(
        &mut self,
        stub: &mut Stub,
        turn: u32,
        step: usize,
        before: StopsLanded,
    ) -> &'static str {
        let thread = self.thread.take().expect("made");
        let until = Instant::now() + TURN_LIMIT;
        // A stop before the thread took its end ends the turn instead, and the end is given again.
        while !stub
            .thread_gone(thread, Duration::from_millis(1))
            .expect("gone")
        {
            if let Some(end) = stub
                .wait_within(thread, turn, Duration::ZERO)
                .expect("look")
            {
                assert_eq!(
                    end,
                    TurnEnd::Stopped {
                        commands_applied: 0
                    },
                    "step {step}"
                );
                let again = stub.begin(thread, &[], Resume::ExitThread).expect("begin");
                assert!(
                    stub.thread_gone(thread, TURN_LIMIT).expect("gone"),
                    "the thread did not end when told again after a stop at step {step}"
                );
                assert_eq!(stub.wait_within(thread, again, Duration::ZERO), Ok(None));
                break;
            }
            assert!(
                Instant::now() < until,
                "the thread did not end after a stop at step {step}"
            );
        }
        stub.release_thread(thread);
        assert_eq!(host_threads(stub, 1), 1, "step {step}");
        if step == 0 {
            ""
        } else {
            landed(before, stub.stops_landed(thread))
        }
    }
}

/// Runs a scenario on a new stub; if it fails, how the stub ended is reported too.
fn run(scenario: &mut impl Scenario) -> BTreeMap<&'static str, usize> {
    let zygote = zygote();
    let mut stub = zygote.fork_stub().expect("fork");
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        every_instruction(scenario, &mut stub)
    })) {
        Ok(windows) => windows,
        Err(failure) => {
            // A stub that ended by itself keeps its own end; a live one is killed.
            let _ = zygote.kill(stub.process());
            eprintln!("the stub: {:?}", zygote.reap(stub.process()));
            std::panic::resume_unwind(failure)
        }
    }
}

#[test]
fn a_stop_at_every_instruction_of_a_turn_of_commands_changes_only_when_it_ends() {
    let windows = run(&mut Commands { made: None });
    for window in ["turn", "publish", "wait"] {
        assert!(
            windows.contains_key(window),
            "no stop landed in the {window} window: {windows:?}"
        );
    }
}

#[test]
fn a_stop_at_every_instruction_before_a_spin_ends_it_after_its_commands() {
    let windows = run(&mut Spin);
    assert!(windows.contains_key("turn"), "{windows:?}");
}

#[test]
fn a_thread_told_to_end_ends_with_a_stop_at_any_instruction() {
    let windows = run(&mut End { thread: None });
    assert!(windows.contains_key("end"), "{windows:?}");
}

/// A turn that enters a piece of code calling a floor entry: the enter's loads, the piece, the entry and its save.
#[cfg(target_arch = "aarch64")]
struct Enter {
    code: Option<wcontainers_executor_host_process::code::CodeArea>,
    loaded: wcontainers_abi::RegisterBlock,
}

#[cfg(target_arch = "aarch64")]
mod enter {
    use wcontainers_arm64::thread_state::{PAGE, SCRATCH};
    use wcontainers_executor_host_process::layout::{
        CODE_AREA, FLOOR_ENTRIES, FLOOR_ENTRY_SIZE, REGISTER_PAGES,
    };

    /// The piece: one call to an entry.
    pub const CALL: usize = CODE_AREA;
    pub const ENTRY: usize = 37;
    pub const INSTRUCTION: usize = size_of::<u32>();

    pub fn call() -> u32 {
        let entry = FLOOR_ENTRIES + ENTRY * FLOOR_ENTRY_SIZE;
        wcontainers_arm64::encode::branch(true, entry as i64 - CALL as i64)
            .expect("an entry within reach")
    }

    /// The block thread 0 holds where the code was entered at `entered` and is at `pc`.
    pub fn at(
        loaded: &wcontainers_abi::RegisterBlock,
        entered: u64,
        pc: u64,
    ) -> wcontainers_abi::RegisterBlock {
        let mut expected = *loaded;
        expected.x[usize::from(SCRATCH)] = entered;
        expected.x[usize::from(PAGE)] = REGISTER_PAGES as u64;
        expected.pc = pc;
        expected
    }
}

#[cfg(target_arch = "aarch64")]
impl Scenario for Enter {
    fn reset(&mut self, stub: &mut Stub) {
        if self.code.is_none() {
            let probe = HostProbe::new().expect("the host is refused");
            let mut code =
                wcontainers_executor_host_process::code::CodeArea::new().expect("code area");
            code.write(&probe, enter::CALL, &[enter::call()]);
            stub.send_block(code.file()).expect("send the code area");
            assert_eq!(
                stub.turn(0, &[Command::MapCode]),
                Ok(TurnEnd::Done {
                    commands_applied: 1
                })
            );
            self.code = Some(code);
        }
        stub.set_registers(0, &self.loaded);
    }

    fn thread(&self) -> usize {
        0
    }

    fn begin(&mut self, stub: &mut Stub) -> u32 {
        stub.begin_enter(0, &[], enter::CALL).expect("enter")
    }

    fn path_ends(&self, stub: &Stub, turn: u32, _: &Stepped, _: usize, waits: bool) -> bool {
        waits
            && stub
                .wait_within(0, turn, Duration::ZERO)
                .expect("look")
                .is_some()
    }

    fn check(
        &mut self,
        stub: &mut Stub,
        turn: u32,
        step: usize,
        before: StopsLanded,
    ) -> &'static str {
        let call = enter::CALL as u64;
        let expected = match wait(stub, 0, turn, step) {
            TurnEnd::Stopped {
                commands_applied: 0,
            } => self.loaded,
            TurnEnd::Interrupted { at } if at == call => enter::at(&self.loaded, call, call),
            TurnEnd::Called { entry } if entry == enter::ENTRY as u64 => {
                let link = call + enter::INSTRUCTION as u64;
                let mut expected = enter::at(&self.loaded, entry, link);
                expected.x[30] = link;
                expected
            }
            other => panic!("after a stop at step {step} the enter ended {other:?}"),
        };
        assert_eq!(
            stub.registers(0),
            expected,
            "the block after a stop at step {step}"
        );
        let where_ = if step == 0 {
            ""
        } else {
            landed(before, settled(stub, 0))
        };
        assert_eq!(
            stub.turn(0, &[]),
            Ok(TurnEnd::Done {
                commands_applied: 0
            }),
            "the thread answers after step {step}"
        );
        where_
    }
}

#[cfg(target_arch = "aarch64")]
#[test]
fn a_stop_at_every_instruction_of_an_enter_and_a_call_saves_what_the_code_held() {
    let mut rng = fastrand::Rng::with_seed(knob("WCHP_SEED", 1) as u64);
    let mut loaded = wcontainers_abi::RegisterBlock::ZERO;
    loaded.x.iter_mut().for_each(|x| *x = rng.u64(..));
    loaded.sp = rng.u64(..);
    loaded.pstate = rng.u64(..) & wcontainers_abi::pstate::NZCV;
    loaded
        .v
        .iter_mut()
        .for_each(|v| *v = [rng.u64(..), rng.u64(..)]);
    loaded.fpsr = rng.u32(..) & wcontainers_arm64::FPSR_BITS;
    loaded.fpcr = rng.u32(..) & wcontainers_arm64::FPCR_BITS;
    loaded.tpidr_el0 = rng.u64(..);
    let windows = run(&mut Enter { code: None, loaded });
    for window in ["turn", "call", "publish", "wait"] {
        assert!(
            windows.contains_key(window),
            "no stop landed in the {window} window: {windows:?}"
        );
    }
}
