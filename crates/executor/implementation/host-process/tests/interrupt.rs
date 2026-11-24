//! An execution's interrupter: an interrupt from another thread reaches a guest that never traps, answered continue it
//! goes on where it was, answered terminate it ends there.
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

/// A guest that adds one to x0 for ever: an add and a branch back to it.
fn spinning(executor: &HostProcess) -> (<HostProcess as Executor>::GuestMemory, Processor) {
    let memory = executor.create_guest_memory().expect("a memory");
    let add = encode::add_immediate(0, 0, 1).expect("an add of one");
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
    let mut executor = executor();
    let (memory, start) = spinning(&executor);
    let (told, heard) = mpsc::channel();
    let mut handler = Interrupts {
        terminate_at: times,
        seen: 0,
        told,
    };
    let began = Instant::now();
    let (execution, interrupter) = executor.execute(start, &memory, &mut handler);
    let interrupting = std::thread::spawn(move || {
        for time in 1..=times {
            std::thread::sleep(PAUSE);
            interrupter.interrupt();
            assert_eq!(
                heard.recv_timeout(Duration::from_secs(5)),
                Ok(time),
                "interrupt {time} reached the handler"
            );
        }
    });
    let stopped = run(execution);
    interrupting.join().expect("the interrupting thread");
    assert_eq!(handler.seen, times, "the interrupts the handler was given");
    assert!(
        began.elapsed() >= PAUSE * times as u32,
        "the guest ran until its last interrupt"
    );
    assert_eq!(stopped.exception, None, "an interrupt is no exception");
    assert!(
        [CODE, CODE + 4].contains(&stopped.program_counter),
        "stopped in the loop, at {:#x}",
        stopped.program_counter
    );
    assert!(
        stopped.general_purpose_registers[0] > 0,
        "the guest counted between its start and its last interrupt"
    );
}

#[test]
fn an_interrupt_answered_terminate_ends_a_guest_that_never_traps() {
    interrupted(1);
}

#[test]
fn interrupts_answered_continue_go_on_and_the_last_ends_it() {
    interrupted(5);
}
