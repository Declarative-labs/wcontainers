//! Where a run looks for an interrupt: no path through translated code runs on without a look.
//!
//! The translator's contract ("Where a thread looks at its stop flag") puts a look at every piece's entry, at every jump
//! back, at every return and after every system call, so between two looks a thread runs the
//! instructions of one piece at most, each at most once. With the progress count kept, as this executor keeps it, the
//! interrupter sets the count's limit to zero and a slice sets it to the slice's end, and both are seen at the same
//! looks. So a slice of a few events stops a run at the first look after them, which these tests state exactly; and an
//! interrupt stops a run that only returns to itself.

use std::num::NonZeroU64;
use std::sync::mpsc;
use std::time::Duration;

use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_executor_wasmtime::{WasmtimeExecutor, WasmtimeTranslatorConfig};
use wcontainers_processor_architecture_arm64::{Processor, ZERO_PROCESSOR};
use wcontainers_translator::{BOUND, LookAhead};
use wcontainers_volatile_bytes::VolatileBytes;

/// RET (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/RET--Return-from-subroutine->).
const RETURN: u32 = 0xD65F_03C0;
/// ADD X0, X0, #1 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value->).
const ADD_ONE: u32 = 0x9100_0400;
/// ADD Xd, Xn, #0 with no registers: a register is copied by adding zero to it.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--immediate---Add-immediate-value->
const COPY: u32 = 0x9100_0000;
/// Where ADD (immediate) names its first source register.
const SOURCE_SHIFT: u32 = 5;
/// BL with no offset (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BL--Branch-with-link->).
const BRANCH_WITH_LINK: u32 = 0x9400_0000;
/// B to itself (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/B--Branch->).
const BRANCH_TO_ITSELF: u32 = 0x1400_0000;
/// The bits of BL's offset in instructions.
const BRANCH_OFFSET_MASK: u32 = (1 << 26) - 1;
/// The link register, X30.
const LINK: usize = 30;
/// The register each function of a chain keeps its return address in: X10 for the first.
const FIRST_KEPT: u32 = 10;
/// The register the code counts its additions in.
const COUNTED: usize = 0;
/// The bytes of one instruction.
const INSTRUCTION_BYTES: u64 = size_of::<u32>() as u64;
/// The pages the code is written on.
const CODE_PAGES: u64 = 4;
/// The additions each called function makes: more than every look-ahead's bound of one look but full's.
const ADDITIONS: usize = 200;
/// The calls the caller makes in a row.
const CALLS: usize = 200;
/// How deep a chain of calls goes below the first function: the functions that call, then the one that returns.
const CHAIN_DEPTH: usize = 6;
/// How long after a run starts its interrupter is used.
const INTERRUPT_AFTER: Duration = Duration::from_millis(50);
/// How long an interrupted run may take to stop before the test fails: far longer than any look takes to come.
const STOP_WITHIN: Duration = Duration::from_secs(10);

/// The look-aheads the executor translates at: each has a bound of its own, and all stop at the same looks.
const LOOK_AHEADS: [LookAhead; 3] = [LookAhead::None, LookAhead::Bounded(BOUND), LookAhead::Full];

/// Counts interrupts and answers terminate to each; any other trap fails the test.
#[derive(Default)]
struct TerminateAtInterrupt {
    interrupts: u64,
}

impl TrapHandler for TerminateAtInterrupt {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse {
        panic!("a system call: {request:?}")
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        panic!("a memory fault at {address:#x} on {access:?}")
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.interrupts += 1;
        InterruptResponse::Terminate
    }
}

/// Returns BL from the instruction at `from` to `to`.
fn branch_with_link(from: u64, to: u64) -> u32 {
    let instructions = to.wrapping_sub(from).cast_signed() / INSTRUCTION_BYTES.cast_signed();
    let offset = u32::try_from(instructions & i64::from(BRANCH_OFFSET_MASK)).expect("26 bits");
    BRANCH_WITH_LINK | offset
}

/// Returns ADD Xd, Xn, #0, which copies Xn into Xd.
fn copy(to: u32, from: u32) -> u32 {
    COPY | (from << SOURCE_SHIFT) | to
}

/// Returns the address of the instruction `index` instructions past the lowest address.
fn at(index: usize) -> u64 {
    LOWEST_ADDRESS + INSTRUCTION_BYTES * index as u64
}

/// A space with the code written from the lowest address on pages that allow reading and running it.
fn space_with(
    executor: &mut WasmtimeExecutor,
    code: &[u32],
) -> <WasmtimeExecutor as Executor>::GuestMemory {
    let (memory, mut table) = executor.create_guest_memory().expect("a space");
    let bytes: Vec<u8> = code.iter().flat_map(|word| word.to_le_bytes()).collect();
    let view = memory
        .borrow(LOWEST_ADDRESS..LOWEST_ADDRESS + bytes.len() as u64)
        .expect("addresses inside the space");
    assert_eq!(view.write(&bytes), bytes.len(), "the code written");
    table
        .set_permissions(
            LOWEST_ADDRESS..LOWEST_ADDRESS + CODE_PAGES * PAGE_SIZE,
            Permissions::READ | Permissions::EXECUTE,
        )
        .expect("the code's pages");
    memory
}

/// Runs the code from the lowest address with the link register given, as the configuration says, on a thread of its
/// own, uses its interrupter after INTERRUPT_AFTER when `interrupt` says so, and returns the state it stopped in and the
/// interrupts it asked about; fails the test if it has not stopped within STOP_WITHIN.
fn run(
    code: &[u32],
    link: u64,
    config: WasmtimeTranslatorConfig,
    interrupt: bool,
) -> (Processor<()>, u64) {
    let code = code.to_vec();
    let (interrupters, interrupter) = mpsc::channel();
    let (stops, stop) = mpsc::channel();
    // A run that never looks keeps its thread forever; the test fails without waiting on it.
    std::thread::spawn(move || {
        let mut executor = WasmtimeExecutor::new(config);
        let memory = space_with(&mut executor, &code);
        let mut start = Processor {
            program_counter: LOWEST_ADDRESS,
            ..ZERO_PROCESSOR
        };
        start.general_purpose_registers[LINK] = link;
        let mut handler = TerminateAtInterrupt::default();
        let (execution, interrupter) = executor.prepare_execution(start, &memory);
        interrupters.send(interrupter).expect("the test waits");
        let ExecutionStop::Terminated(stopped) = execution.run(&mut handler) else {
            panic!("the executor waited");
        };
        let _ = stops.send((stopped, handler.interrupts));
    });
    let interrupter = interrupter.recv().expect("the run's interrupter");
    if interrupt {
        std::thread::sleep(INTERRUPT_AFTER);
        interrupter.interrupt();
    }
    stop.recv_timeout(STOP_WITHIN).unwrap_or_else(|_| {
        panic!(
            "{:?}: the run did not stop within {STOP_WITHIN:?}",
            config.look_ahead
        )
    })
}

/// Runs the code as [`run`] does at the look-ahead with a slice of `slice` events and no interrupt.
fn run_sliced(code: &[u32], link: u64, look_ahead: LookAhead, slice: u64) -> (Processor<()>, u64) {
    let slice = Some(NonZeroU64::new(slice).expect("a slice"));
    run(
        code,
        link,
        WasmtimeTranslatorConfig { look_ahead, slice },
        false,
    )
}

/// Runs the code as [`run`] does at the look-ahead with no slice, interrupted.
fn run_interrupted(code: &[u32], link: u64, look_ahead: LookAhead) -> (Processor<()>, u64) {
    let slice = None;
    run(
        code,
        link,
        WasmtimeTranslatorConfig { look_ahead, slice },
        true,
    )
}

/// One RET with the link register at its own address: every turn returns where no caller waits.
#[test]
fn a_return_to_itself_stops_for_an_interrupt() {
    for look_ahead in LOOK_AHEADS {
        let (stopped, interrupts) = run_interrupted(&[RETURN], LOWEST_ADDRESS, look_ahead);
        assert_eq!(
            (stopped.program_counter, stopped.exception, interrupts),
            (LOWEST_ADDRESS, None, 1),
            "{look_ahead:?}: where the interrupted run stopped, and the interrupts asked"
        );
    }
}

/// The RET of a slice of one event is the slice's end: the run stops before it returns again.
#[test]
fn a_return_to_itself_stops_at_the_look_after_its_slice() {
    for look_ahead in LOOK_AHEADS {
        let (stopped, interrupts) = run_sliced(&[RETURN], LOWEST_ADDRESS, look_ahead, 1);
        assert_eq!(
            (stopped.program_counter, interrupts),
            (LOWEST_ADDRESS, 1),
            "{look_ahead:?}: the first look after the one return is at the piece it returned to"
        );
    }
}

/// A caller makes CALLS calls of a function that adds ADDITIONS times and returns; the first call is the slice's one
/// event, so the run stops at the entry of the function it called, before its first addition.
#[test]
fn a_function_called_after_the_slice_runs_none_of_its_instructions() {
    let function = at(CALLS + 1);
    let mut code: Vec<u32> = (0..CALLS)
        .map(|call| branch_with_link(at(call), function))
        .collect();
    code.push(BRANCH_TO_ITSELF);
    code.extend([ADD_ONE; ADDITIONS]);
    code.push(RETURN);
    for look_ahead in LOOK_AHEADS {
        let (stopped, interrupts) = run_sliced(&code, u64::MIN, look_ahead, 1);
        assert_eq!(
            (
                stopped.program_counter,
                stopped.general_purpose_registers[COUNTED],
                stopped.general_purpose_registers[LINK],
                interrupts
            ),
            (function, 0, at(1), 1),
            "{look_ahead:?}: the pc, the additions, the return address and the interrupts"
        );
    }
}

/// A chain of calls CHAIN_DEPTH deep: each function keeps its return address, calls the next, adds ADDITIONS times
/// and returns, and the last only returns. The slice ends at that last return, so the run stops right after the call
/// it returns from, before any caller adds.
#[test]
fn callers_returned_to_after_the_slice_run_none_of_their_instructions() {
    // The first function calls the chain and then branches to itself.
    let mut code = vec![u32::MIN, BRANCH_TO_ITSELF];
    let length = ADDITIONS + 4;
    let start_of = |depth: usize| at(2 + (depth - 1) * length);
    code[0] = branch_with_link(at(0), start_of(1));
    for depth in 1..CHAIN_DEPTH {
        let kept = FIRST_KEPT + depth as u32;
        let call = start_of(depth) + INSTRUCTION_BYTES;
        code.push(copy(kept, LINK as u32));
        code.push(branch_with_link(call, start_of(depth + 1)));
        code.extend([ADD_ONE; ADDITIONS]);
        code.push(copy(LINK as u32, kept));
        code.push(RETURN);
    }
    code.push(RETURN);
    // The events: a call into each function of the chain, then the last one's return.
    let events = CHAIN_DEPTH as u64 + 1;
    let deepest_caller = start_of(CHAIN_DEPTH - 1);
    let after_its_call = deepest_caller + 2 * INSTRUCTION_BYTES;
    for look_ahead in LOOK_AHEADS {
        let (stopped, interrupts) = run_sliced(&code, u64::MIN, look_ahead, events);
        assert_eq!(
            (
                stopped.program_counter,
                stopped.general_purpose_registers[COUNTED],
                interrupts
            ),
            (after_its_call, 0, 1),
            "{look_ahead:?}: the pc, the additions and the interrupts"
        );
    }
}
