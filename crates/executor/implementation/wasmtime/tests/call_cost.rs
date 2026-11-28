//! What one guest call of a function that only returns costs, with its looks for an interrupt.
//!
//! A loop calls such a function each round, timed against the same loop without the call.

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

/// BL four instructions on (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BL--Branch-with-link->).
const CALL_FOUR_ON: u32 = 0x9400_0004;
/// SUBS X2, X2, #1, which counts the rounds down.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUBS--immediate---Subtract-immediate-value--setting-flags->
const COUNT_DOWN: u32 = 0xF100_0442;
/// `B.NE` one instruction back (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B-cond--Branch-conditionally->).
const AGAIN_ONE_BACK: u32 = 0x54FF_FFE1;
/// `B.NE` two instructions back (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B-cond--Branch-conditionally->).
const AGAIN_TWO_BACK: u32 = 0x54FF_FFC1;
/// SVC #0 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SVC--Supervisor-call->).
const SUPERVISOR_CALL: u32 = 0xD400_0001;
/// RET (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/RET--Return-from-subroutine->).
const RETURN: u32 = 0xD65F_03C0;
/// The register the rounds are counted down in.
const ROUNDS_REGISTER: usize = 2;
/// The rounds each timed run makes.
const ROUNDS: u64 = 50_000_000;
/// The rounds made before timing starts, which translate and warm the code.
const WARM_ROUNDS: u64 = 100_000;
/// The timed runs; the fastest is reported, as the one least disturbed by other work on the machine.
const RUNS: usize = 5;

/// Ends the run at the first system call.
struct EndAtCall;

impl TrapHandler for EndAtCall {
    fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        panic!("a memory fault at {address:#x} on {access:?}")
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        InterruptResponse::Continue
    }
}

/// Runs the code from the lowest address with the rounds given until its system call, and returns how long it took.
fn run(
    executor: &mut WasmtimeExecutor,
    memory: &<WasmtimeExecutor as Executor>::GuestMemory,
    rounds: u64,
) -> Duration {
    let mut start = Processor {
        program_counter: LOWEST_ADDRESS,
        ..ZERO_PROCESSOR
    };
    start.general_purpose_registers[ROUNDS_REGISTER] = rounds;
    let started = Instant::now();
    let (execution, _interrupter) = executor.prepare_execution(start, memory);
    let ExecutionStop::Terminated(stopped) = execution.run(&mut EndAtCall) else {
        panic!("the executor waited");
    };
    assert_eq!(
        stopped.general_purpose_registers[ROUNDS_REGISTER], 0,
        "every round made"
    );
    started.elapsed()
}

/// Returns the fastest of the timed runs of the code in a new space.
fn fastest(code: &[u32]) -> Duration {
    let mut executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let bytes: Vec<u8> = code.iter().flat_map(|word| word.to_le_bytes()).collect();
    let view = memory
        .borrow(LOWEST_ADDRESS..LOWEST_ADDRESS + bytes.len() as u64)
        .expect("addresses inside the space");
    assert_eq!(view.write(&bytes), bytes.len(), "the code written");
    table
        .set_permissions(
            LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
            Permissions::READ | Permissions::EXECUTE,
        )
        .expect("the code's page");
    run(&mut executor, &memory, WARM_ROUNDS);
    (0..RUNS)
        .map(|_| run(&mut executor, &memory, ROUNDS))
        .min()
        .expect("at least one run")
}

#[test]
#[ignore = "a measurement, run on its own: cargo test --release --test call_cost -- --ignored --nocapture"]
fn a_call_of_a_function_that_only_returns() {
    let empty = fastest(&[COUNT_DOWN, AGAIN_ONE_BACK, SUPERVISOR_CALL]);
    let calling = fastest(&[
        CALL_FOUR_ON,
        COUNT_DOWN,
        AGAIN_TWO_BACK,
        SUPERVISOR_CALL,
        RETURN,
    ]);
    let per_round = |time: Duration| time.as_nanos() as f64 / ROUNDS as f64;
    eprintln!(
        "CALL COST empty loop {:.2} ns per round, calling loop {:.2} ns per round, {:.2} ns per call and return \
         (the fastest of {RUNS} runs of {ROUNDS} rounds)",
        per_round(empty),
        per_round(calling),
        per_round(calling.saturating_sub(empty)),
    );
}
