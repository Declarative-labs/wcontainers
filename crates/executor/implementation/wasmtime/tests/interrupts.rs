//! An interrupter reaches its own execution only: before its first poll, from another thread while it runs, and
//! not at all once it ended.

use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_executor::memory_space::{MemorySpace, VolatileBytes};
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_conformance::GuestArchitecture;
use wcontainers_executor_wasmtime::{Arm64Executor, TranslatorConfig};
use wcontainers_processor_architecture_arm64::{Arm64, Processor};

/// Where the guest's code is placed.
const CODE: u64 = LOWEST_ADDRESS;
/// A count the guest does not finish counting down before it is interrupted.
const ENDLESS: u64 = u64::MAX;
/// A count the guest finishes at once.
const SHORT: u64 = u8::MAX as u64;
/// How long the other thread waits before it interrupts the running guest.
const WAIT: Duration = Duration::from_millis(50);

/// Counts the interrupts it is given and answers each as it was made to.
struct Interrupts {
    count: usize,
    answer: fn() -> InterruptResponse,
}

impl TrapHandler for Interrupts {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse {
        panic!("a system call {request:?}")
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        panic!("a memory fault at {address:#x} on {access:?}")
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.count += 1;
        (self.answer)()
    }
}

/// Returns a space holding the counting guest from the count, and its start state.
fn counting_guest(
    executor: &Arm64Executor,
    count: u64,
) -> (<Arm64Executor as Executor>::GuestMemory, Processor) {
    let memory = executor.create_guest_memory().expect("a space");
    let guest = Arm64::build_count_down(CODE, count);
    let end = CODE + guest.code.len() as u64;
    assert_eq!(memory.borrow(CODE..end).expect("code").write(&guest.code), guest.code.len());
    let code_page = CODE..CODE + PAGE_SIZE;
    memory
        .set_permissions(code_page, Permissions::READ | Permissions::EXECUTE)
        .expect("the code page");
    (memory, guest.start)
}

/// Returns the output of an execution polled once, which must complete it.
fn complete<F: Future>(execution: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    match pin!(execution).poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the execution was not ready at its first poll"),
    }
}

#[test]
fn an_interrupt_before_the_first_poll_is_handled_at_the_start() {
    let mut executor = Arm64Executor::new(TranslatorConfig::default());
    let (memory, start) = counting_guest(&executor, ENDLESS);
    let mut handler = Interrupts {
        count: usize::MIN,
        answer: || InterruptResponse::Terminate,
    };
    let (execution, interrupter) = executor.execute(start.clone(), &memory, &mut handler);
    interrupter.interrupt();
    let stopped = complete(execution);
    assert_eq!(handler.count, 1);
    assert_eq!(stopped, start, "terminated before any instruction ran");
}

#[test]
fn an_interrupt_from_another_thread_reaches_the_running_guest() {
    let mut executor = Arm64Executor::new(TranslatorConfig::default());
    let (memory, start) = counting_guest(&executor, ENDLESS);
    let mut handler = Interrupts {
        count: usize::MIN,
        answer: || InterruptResponse::Terminate,
    };
    let (execution, interrupter) = executor.execute(start.clone(), &memory, &mut handler);
    let raiser = std::thread::spawn(move || {
        std::thread::sleep(WAIT);
        interrupter.interrupt();
    });
    let stopped = complete(execution);
    raiser.join().expect("the interrupting thread");
    assert_eq!(handler.count, 1);
    assert_eq!(stopped.exception, None, "an interrupt is no exception");
    assert!(
        stopped.general_purpose_registers[0] < ENDLESS,
        "the guest counted before it was interrupted"
    );
}

#[test]
fn an_interrupt_after_the_end_reaches_no_execution() {
    let mut executor = Arm64Executor::new(TranslatorConfig::default());
    let (memory, start) = counting_guest(&executor, SHORT);
    let mut handler = Interrupts {
        count: usize::MIN,
        answer: || InterruptResponse::Continue,
    };
    let (execution, first) = executor.execute(start.clone(), &memory, &mut handler);
    let ended = complete(execution);
    first.interrupt();
    let (execution, _second) = executor.execute(start, &memory, &mut handler);
    assert_eq!(complete(execution), ended, "the second execution ends as the first");
    assert_eq!(handler.count, usize::MIN, "no execution was interrupted");
}
