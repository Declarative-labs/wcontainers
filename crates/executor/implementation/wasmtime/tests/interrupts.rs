//! An interrupter reaches its own execution only: before its first poll, while it runs, never once it ended.

use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use wcontainers_abi::LOWEST_ADDRESS as CODE;
use wcontainers_abi::{Access, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_arm64::encode::{self, Register, form};
use wcontainers_executor::memory_space::{MemorySpace, VolatileBytes};
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_wasmtime::{Arm64Executor, TranslatorConfig};
use wcontainers_processor_architecture_arm64::{INSTRUCTION_BYTES, Processor, ZERO_PROCESSOR};

/// The register the guest counts down in, x0.
const COUNTER: Register = Register::MIN;
/// What the guest adds to its count each time round.
const STEP: i64 = -1;
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

/// Returns a space holding a guest that counts down from the count and then stops at a breakpoint, and its start state.
fn counting_guest(
    executor: &Arm64Executor,
    count: u64,
) -> (<Arm64Executor as Executor>::GuestMemory, Processor) {
    let memory = executor.create_guest_memory().expect("a space");
    let back = -i64::try_from(INSTRUCTION_BYTES).expect("an instruction's size fits");
    let instructions = [
        encode::add_immediate(COUNTER, COUNTER, STEP).expect("a step encodes"),
        encode::compare_branch(true, COUNTER, back).expect("a branch back encodes"),
        encode::exception(form::BRK, u16::MIN),
    ];
    let code: Vec<u8> = instructions
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    let end = CODE + code.len() as u64;
    assert_eq!(
        memory.borrow(CODE..end).expect("code").write(&code),
        code.len()
    );
    let code_page = CODE..CODE + PAGE_SIZE;
    memory
        .set_permissions(code_page, Permissions::READ | Permissions::EXECUTE)
        .expect("the code page");
    let mut start = Processor {
        program_counter: CODE,
        ..ZERO_PROCESSOR
    };
    start.general_purpose_registers[usize::from(COUNTER)] = count;
    (memory, start)
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
        stopped.general_purpose_registers[usize::from(COUNTER)] < ENDLESS,
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
    assert_eq!(
        complete(execution),
        ended,
        "the second execution ends as the first"
    );
    assert_eq!(handler.count, usize::MIN, "no execution was interrupted");
}
