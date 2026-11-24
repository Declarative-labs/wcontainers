//! An execution's interrupter: an interrupt from another thread reaches a guest that never traps, answered continue it
//! goes on where it was, answered terminate it ends there, whenever it is sent.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::pin::pin;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use wcontainers_abi::{Access, Permissions, SystemCallRequest};
use wcontainers_arm64::encode;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_host_process::executor::HostProcess;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_processor_architecture_arm64::{Processor, ZERO_PROCESSOR};
use wcontainers_volatile_bytes::VolatileBytes;

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

fn executor() -> HostProcess {
    HostProcess::new(Arc::clone(HOST.get_or_init(|| {
        Arc::new(HostProcessHost::new().expect("the host is refused"))
    })))
}

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
) -> (<HostProcess as Executor>::GuestMemory, Processor) {
    let memory = executor.create_guest_memory().expect("a memory");
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
    memory
        .set_permissions(CODE..CODE + PAGE, Permissions::READ | Permissions::EXECUTE)
        .expect("code permissions");
    let start = Processor {
        program_counter: CODE,
        ..ZERO_PROCESSOR
    };
    (memory, start)
}

/// Polls the execution to its end on this thread.
fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
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
    let (execution, interrupter) = executor.execute(start, &memory, &mut handler);
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
    let stopped = run(execution);
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
