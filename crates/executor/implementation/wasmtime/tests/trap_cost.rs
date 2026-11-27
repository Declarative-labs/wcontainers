//! What one trap costs: a guest that makes a system call and branches back to it, each call answered by the handler
//! from inside the run, timed over many calls.

use std::time::{Duration, Instant};

use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionStop, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_wasmtime::{WasmtimeExecutor, WasmtimeTranslatorConfig};
use wcontainers_processor_architecture_arm64::{Processor, ZERO_PROCESSOR};
use wcontainers_volatile_bytes::VolatileBytes;

/// SVC #0 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SVC--Supervisor-call->).
const SUPERVISOR_CALL: u32 = 0xD400_0001;
/// B with an offset of minus one instruction, back to the call
/// (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B--Branch->).
const BRANCH_BACK: u32 = 0x17FF_FFFF;
/// The calls each timed run answers.
const CALLS: u64 = 1_000_000;
/// The calls answered before timing starts, which translate and warm the code.
const WARM_CALLS: u64 = 10_000;
/// The timed runs; the fastest is reported, as the one least disturbed by other work on the machine.
const RUNS: usize = 5;

/// Answers every system call with zero until it has answered the count, then ends the run.
struct Counting {
    answered: u64,
    count: u64,
}

impl TrapHandler for Counting {
    fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
        if self.answered == self.count {
            return SystemCallResponse::Terminate;
        }
        self.answered += 1;
        SystemCallResponse::Return(0)
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        panic!("a memory fault at {address:#x} on {access:?}")
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        InterruptResponse::Continue
    }
}

/// Runs the guest from its start until the handler has answered the count, and returns how long it took.
fn run(
    executor: &mut WasmtimeExecutor,
    memory: &<WasmtimeExecutor as Executor>::GuestMemory,
    count: u64,
) -> Duration {
    let start = Processor {
        program_counter: LOWEST_ADDRESS,
        ..ZERO_PROCESSOR
    };
    let mut handler = Counting { answered: 0, count };
    let started = Instant::now();
    let (execution, _interrupter) = executor.prepare_execution(start, memory);
    let ExecutionStop::Terminated(_) = execution.run(&mut handler) else {
        panic!("the executor waited");
    };
    let elapsed = started.elapsed();
    assert_eq!(handler.answered, count, "the calls answered");
    elapsed
}

#[test]
#[ignore = "a measurement, run on its own: cargo test --release --test trap_cost -- --ignored --nocapture"]
fn system_call_answered_inside_the_run() {
    let mut executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let code: Vec<u8> = [SUPERVISOR_CALL, BRANCH_BACK]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    let bytes = memory
        .borrow(LOWEST_ADDRESS..LOWEST_ADDRESS + code.len() as u64)
        .expect("addresses inside the space");
    assert_eq!(bytes.write(&code), code.len(), "the code written");
    table
        .set_permissions(
            LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
            Permissions::READ | Permissions::EXECUTE,
        )
        .expect("the code's page");
    run(&mut executor, &memory, WARM_CALLS);
    let fastest = (0..RUNS)
        .map(|_| run(&mut executor, &memory, CALLS))
        .min()
        .expect("at least one run");
    let nanoseconds = fastest.as_nanos() as f64 / CALLS as f64;
    eprintln!(
        "TRAP COST: {nanoseconds:.1} ns per system call, the fastest of {RUNS} runs of {CALLS} calls"
    );
}
