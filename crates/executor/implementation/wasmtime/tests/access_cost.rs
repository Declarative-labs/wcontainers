//! What one guest load or store costs below 4 GiB and on the top page of the 2^48 bytes of addresses.
//!
//! A loop loads, adds to and stores a word, timed against the same loop without the accesses.

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

/// LDR X1, [X0] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate-->).
const LOAD: u32 = 0xF940_0001;
/// ADD X1, X1, #1 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value->).
const ADD_ONE: u32 = 0x9100_0421;
/// STR X1, [X0] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-register--immediate-->).
const STORE: u32 = 0xF900_0001;
/// AND X5, X2, #0x1FF, the word of the page this round uses.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/AND--immediate---Bitwise-AND--immediate->
const WORD_OF_PAGE: u32 = 0x9240_2045;
/// LDR X1, [X0, X5, LSL #3] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--register---Load-register--register-->).
const LOAD_INDEXED: u32 = 0xF865_7801;
/// STR X1, [X0, X5, LSL #3] (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--register---Store-register--register-->).
const STORE_INDEXED: u32 = 0xF825_7801;
/// SUBS X2, X2, #1, which counts the rounds down.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUBS--immediate---Subtract-immediate-value--setting-flags->
const COUNT_DOWN: u32 = 0xF100_0442;
/// `B.NE` one instruction back, to the loop's first instruction.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B-cond--Branch-conditionally->
const AGAIN_ONE_BACK: u32 = 0x54FF_FFE1;
/// `B.NE` four instructions back, to the loop's first instruction.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B-cond--Branch-conditionally->
const AGAIN_FOUR_BACK: u32 = 0x54FF_FF81;
/// `B.NE` five instructions back, to the loop's first instruction.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B-cond--Branch-conditionally->
const AGAIN_FIVE_BACK: u32 = 0x54FF_FF61;
/// SVC #0 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SVC--Supervisor-call->).
const SUPERVISOR_CALL: u32 = 0xD400_0001;
/// The guest's 64-bit words in a page.
const WORDS_PER_PAGE: u64 = PAGE_SIZE / size_of::<u64>() as u64;
/// The rounds each timed run makes.
const ROUNDS: u64 = 50_000_000;
/// The rounds made before timing starts, which translate and warm the code.
const WARM_ROUNDS: u64 = 100_000;
/// The timed runs; the fastest is reported, as the one least disturbed by other work on the machine.
const RUNS: usize = 5;
/// The accesses of one round: a load and a store.
const ACCESSES_PER_ROUND: u64 = 2;
/// The lowest address above the addresses a 32-bit offset reaches.
const ABOVE_FOUR_GIB: u64 = 1 << u32::BITS;
/// The addresses a space has: those of a 48-bit virtual address.
const SPACE_END: u64 = 1 << 48;
/// Where the loops' data goes below 4 GiB: sixteen pages past the code.
const LOW_DATA: u64 = LOWEST_ADDRESS + 16 * PAGE_SIZE;

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

/// One measured loop: its code and the data page it uses.
struct Case {
    name: &'static str,
    code: &'static [u32],
    data: u64,
}

/// Runs the code with X0 the data page and X2 the rounds until its system call, and returns how long it took.
fn run(
    executor: &mut WasmtimeExecutor,
    memory: &<WasmtimeExecutor as Executor>::GuestMemory,
    data: u64,
    rounds: u64,
) -> Duration {
    let mut registers = ZERO_PROCESSOR.general_purpose_registers;
    registers[0] = data;
    registers[2] = rounds;
    let start = Processor {
        program_counter: LOWEST_ADDRESS,
        general_purpose_registers: registers,
        ..ZERO_PROCESSOR
    };
    let started = Instant::now();
    let (execution, _interrupter) = executor.prepare_execution(start, memory);
    let ExecutionStop::Terminated(_) = execution.run(&mut EndAtCall) else {
        panic!("the executor waited");
    };
    started.elapsed()
}

/// Returns the fastest of the timed runs of the case in a new space, after checking the words it added up to.
fn fastest(case: &Case) -> Option<Duration> {
    let mut executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let code: Vec<u8> = case
        .code
        .iter()
        .chain(&[SUPERVISOR_CALL])
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
    if let Err(error) = table.set_permissions(
        case.data..case.data + PAGE_SIZE,
        Permissions::READ | Permissions::WRITE,
    ) {
        eprintln!(
            "ACCESS COST {}: the data page is refused: {error:?}",
            case.name
        );
        return None;
    }
    run(&mut executor, &memory, case.data, WARM_ROUNDS);
    let fastest = (0..RUNS)
        .map(|_| run(&mut executor, &memory, case.data, ROUNDS))
        .min()
        .expect("at least one run");
    let mut page = vec![0; usize::try_from(PAGE_SIZE).expect("a page")];
    let data = memory
        .borrow(case.data..case.data + PAGE_SIZE)
        .expect("the data page");
    assert_eq!(data.read(&mut page), page.len(), "the data read");
    let total: u64 = page
        .as_chunks::<{ size_of::<u64>() }>()
        .0
        .iter()
        .map(|word| u64::from_le_bytes(*word))
        .sum();
    let empty = u64::from(case.code.len() == 2);
    assert_eq!(
        total,
        (1 - empty) * (WARM_ROUNDS + RUNS as u64 * ROUNDS),
        "{}: the words added up",
        case.name
    );
    Some(fastest)
}

#[test]
#[ignore = "a measurement, run on its own: cargo test --release --test access_cost -- --ignored --nocapture"]
fn load_and_store_below_four_gib_and_on_the_top_page() {
    let top_page = SPACE_END - PAGE_SIZE;
    let above = ABOVE_FOUR_GIB + 16 * PAGE_SIZE;
    let same_word: &[u32] = &[LOAD, ADD_ONE, STORE, COUNT_DOWN, AGAIN_FOUR_BACK];
    let each_word: &[u32] = &[
        WORD_OF_PAGE,
        LOAD_INDEXED,
        ADD_ONE,
        STORE_INDEXED,
        COUNT_DOWN,
        AGAIN_FIVE_BACK,
    ];
    let empty = Case {
        name: "empty loop",
        code: &[COUNT_DOWN, AGAIN_ONE_BACK],
        data: LOW_DATA,
    };
    let empty_time = fastest(&empty).expect("the empty loop runs");
    eprintln!(
        "ACCESS COST empty loop: {:.2} ns per round",
        empty_time.as_nanos() as f64 / ROUNDS as f64
    );
    let cases = [
        ("same word", same_word, LOW_DATA),
        ("same word", same_word, above),
        ("same word", same_word, top_page),
        ("each word of the page", each_word, LOW_DATA),
        ("each word of the page", each_word, above),
        ("each word of the page", each_word, top_page),
    ];
    for (name, code, data) in cases {
        let case = Case { name, code, data };
        let Some(time) = fastest(&case) else {
            continue;
        };
        let round = time.as_nanos() as f64 / ROUNDS as f64;
        let access = (time.saturating_sub(empty_time)).as_nanos() as f64
            / (ROUNDS * ACCESSES_PER_ROUND) as f64;
        eprintln!(
            "ACCESS COST {name} at {data:#x}: {round:.2} ns per round, {access:.2} ns per access over the empty loop \
             (the fastest of {RUNS} runs of {ROUNDS} rounds, {WORDS_PER_PAGE} words in a page)"
        );
    }
}
