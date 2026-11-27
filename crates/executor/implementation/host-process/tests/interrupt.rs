//! An execution's interrupter: an interrupt from another thread reaches a guest that never traps, answered continue it
//! goes on where it was, answered terminate it ends there, whenever it is sent.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use wcontainers_abi::{Access, Permissions, SystemCallRequest};
use wcontainers_arm64::encode;
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_executor_host_process::executor::{HostProcess, HostProcessConfig};
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_processor_architecture_arm64::{Processor, ZERO_PROCESSOR};
use wcontainers_volatile_bytes::VolatileBytes;

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

/// An executor whose slice is longer than any test here runs, so that every interrupt its handlers are given is the
/// interrupter's.
fn executor() -> HostProcess {
    let host = Arc::clone(
        HOST.get_or_init(|| Arc::new(HostProcessHost::new().expect("the host is refused"))),
    );
    HostProcess::with_config(host, HostProcessConfig { slice: NO_SLICE })
}

/// Longer than any test here runs.
const NO_SLICE: Duration = Duration::from_secs(3600);

const CODE: u64 = 0x40_0000;
const PAGE: u64 = 4096;
/// How long the interrupting thread waits before each interrupt: long enough for the guest to be running.
const PAUSE: Duration = Duration::from_millis(50);
/// The largest step an add's twelve-bit immediate holds.
const MOST_STEP: i64 = (1 << 12) - 1;
/// How long an interrupt may take to reach the handler.
const REACHED_WITHIN: Duration = Duration::from_secs(2);

/// Answers interrupts continue until the last, which it answers terminate, telling the test of each.
struct Interrupts {
    terminate_at: usize,
    seen: usize,
    told: mpsc::Sender<usize>,
}

impl TrapHandler for Interrupts {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse {
        panic!("the guest makes no system call, yet made {request:?}")
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        panic!("the guest touches no memory, yet faulted at {address:#x} ({access:?})")
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.seen += 1;
        let _ = self.told.send(self.seen);
        if self.seen == self.terminate_at {
            InterruptResponse::Terminate
        } else {
            InterruptResponse::Continue
        }
    }
}

/// A guest that adds `step` to x0 for ever: an add and a branch back to it.
fn spinning(
    executor: &HostProcess,
    step: i64,
) -> (<HostProcess as Executor>::GuestMemory, Processor<()>) {
    let (memory, mut table) = executor.create_guest_memory().expect("a memory");
    let add = encode::add_immediate(0, 0, step).expect("an add of a step");
    let back = encode::branch(false, -4).expect("a branch back");
    let code: Vec<u8> = [add, back].iter().flat_map(|w| w.to_le_bytes()).collect();
    assert_eq!(
        memory
            .borrow(CODE..CODE + code.len() as u64)
            .expect("code")
            .write(&code),
        code.len()
    );
    table
        .set_permissions(CODE..CODE + PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code permissions");
    let start = Processor {
        program_counter: CODE,
        ..ZERO_PROCESSOR
    };
    (memory, start)
}

/// The processor an execution ended with: form P never waits.
fn ended<Wait>(stop: ExecutionStop<Processor<()>, Wait>) -> Processor<()> {
    match stop {
        ExecutionStop::Terminated(processor) => processor,
        ExecutionStop::Waiting { .. } => panic!("form P waited"),
    }
}

fn interrupted(times: usize) {
    let pauses = vec![PAUSE; times];
    let began = Instant::now();
    let counted = interrupted_after(&mut executor(), 1, &pauses)
        .expect("every interrupt reached the handler");
    assert!(
        counted > 0,
        "the guest counted between its start and its last interrupt"
    );
    assert!(
        began.elapsed() >= PAUSE * times as u32,
        "the guest ran until its last interrupt"
    );
}

/// Runs the guest spinning by `step`, sending one interrupt after each pause, the handler ending it at the last; the
/// guest's x0 at its end, or the first interrupt that did not reach the handler within [`REACHED_WITHIN`] (a further
/// one then frees it).
fn interrupted_after(
    executor: &mut HostProcess,
    step: i64,
    pauses: &[Duration],
) -> Result<u64, usize> {
    let times = pauses.len();
    let (memory, start) = spinning(executor, step);
    let (told, heard) = mpsc::channel();
    let mut handler = Interrupts {
        terminate_at: times,
        seen: 0,
        told,
    };
    let (execution, interrupter) = executor.prepare_execution(start, &memory);
    let pauses = pauses.to_vec();
    let interrupting = std::thread::spawn(move || {
        let mut lost = None;
        for (time, pause) in (1..=times).zip(pauses) {
            let until = Instant::now() + pause;
            while Instant::now() < until {
                std::hint::spin_loop();
            }
            interrupter.interrupt();
            while heard.recv_timeout(REACHED_WITHIN) != Ok(time) {
                lost.get_or_insert(time);
                interrupter.interrupt();
            }
        }
        lost
    });
    let stopped = ended(execution.run(&mut handler));
    let lost = interrupting.join().expect("the interrupting thread");
    assert_eq!(handler.seen, times, "the interrupts the handler was given");
    assert_eq!(stopped.exception, None, "an interrupt is no exception");
    assert!(
        [CODE, CODE + 4].contains(&stopped.program_counter),
        "stopped in the loop, at {:#x}",
        stopped.program_counter
    );
    let counted = stopped.general_purpose_registers[0];
    assert!(
        counted.is_multiple_of(step as u64),
        "the guest's x0 is {counted}, which no number of adds of {step} make"
    );
    lost.map_or(Ok(counted), Err)
}

#[test]
fn an_interrupt_answered_terminate_ends_a_guest_that_never_traps() {
    interrupted(1);
}

#[test]
fn interrupts_answered_continue_go_on_and_the_last_ends_it() {
    interrupted(5);
}

/// One to three interrupts each after a seeded pause of 0 to 300 us, spun so that they reach the run at every early moment
/// (the first piece translated or loaded, the thread between turns or not yet in its turn), the guest's step seeded so
/// that its code is translated anew; every eighth case on a new executor. Knobs: `WCHP_SEED`, `WCHP_INTERRUPT_CASES`
/// (default 400).
#[test]
fn an_interrupt_at_any_early_moment_reaches_the_handler() {
    let seed = knob("WCHP_SEED", 1);
    let cases = knob("WCHP_INTERRUPT_CASES", 400);
    let mut random = fastrand::Rng::with_seed(seed);
    let mut executor = executor();
    let mut lost = Vec::new();
    for case in 0..cases {
        if case % 8 == 0 {
            executor = self::executor();
        }
        let pauses: Vec<Duration> = (0..random.usize(1..=3))
            .map(|_| Duration::from_micros(random.u64(0..=300)))
            .collect();
        let step = random.i64(1..=MOST_STEP);
        if let Err(time) = interrupted_after(&mut executor, step, &pauses) {
            lost.push((case, time, pauses));
        }
    }
    println!("seed {seed}: {cases} cases, interrupts lost {lost:?}");
    assert!(
        lost.is_empty(),
        "interrupts lost (case, which, pauses): {lost:?}"
    );
}

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |value| value.parse().expect(name))
}

/// A guest that stores x0 into the writable and executable page its code is on, then spins, is ended by an interrupt in
/// its spin with the word stored, as on Linux.
#[test]
fn a_store_into_the_page_its_own_code_is_on_completes_before_an_interrupt_ends_the_spin() {
    const STORED: u64 = 0x1234_5678;
    let at = CODE + PAGE / 2;
    let mut executor = executor();
    let (memory, mut table) = executor.create_guest_memory().expect("a memory");
    let store =
        encode::load_store(encode::form::STR, 0, 3, 0, size_of::<u64>() as u64).expect("a store");
    let spin = encode::branch(false, 0).expect("a branch to itself");
    let code: Vec<u8> = [store, spin].iter().flat_map(|w| w.to_le_bytes()).collect();
    assert_eq!(
        memory
            .borrow(CODE..CODE + code.len() as u64)
            .expect("code")
            .write(&code),
        code.len()
    );
    let all = Permissions::READ | Permissions::WRITE | Permissions::EXECUTE;
    table
        .set_permissions(CODE..CODE + PAGE, all)
        .expect("code permissions");
    let mut start = Processor {
        program_counter: CODE,
        ..ZERO_PROCESSOR
    };
    start.general_purpose_registers[0] = STORED;
    start.general_purpose_registers[3] = at;
    let (told, heard) = mpsc::channel();
    let mut handler = Interrupts {
        terminate_at: 1,
        seen: 0,
        told,
    };
    let (execution, interrupter) = executor.prepare_execution(start, &memory);
    let interrupting = std::thread::spawn(move || {
        std::thread::sleep(PAUSE);
        interrupter.interrupt();
        while heard.recv_timeout(REACHED_WITHIN).is_err() {
            interrupter.interrupt();
        }
    });
    let stopped = ended(execution.run(&mut handler));
    interrupting.join().expect("the interrupting thread");
    let mut word = [0; size_of::<u64>()];
    let length = word.len();
    assert_eq!(
        memory
            .borrow(at..at + length as u64)
            .expect("the word")
            .read(&mut word),
        length
    );
    assert_eq!(stopped.exception, None, "an interrupt is no exception");
    assert_eq!(
        stopped.program_counter,
        CODE + 4,
        "ended in the spin, the store done"
    );
    assert_eq!(u64::from_le_bytes(word), STORED);
}
