//! The executor's own bound on a run, and the counter read that traps for the kernel to answer.
//!
//! After each slice of progress events its configuration sets, the run asks the handler as at an outside interrupt.

use std::num::NonZeroU64;

use wcontainers_abi::{Access, LOWEST_ADDRESS, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionStop, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_executor_wasmtime::{WasmtimeExecutor, WasmtimeTranslatorConfig};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Exception, Processor, SystemAccess, ZERO_PROCESSOR,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// SUB X0, X0, #1 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUB--immediate---Subtract-immediate-value->).
const SUBTRACT_ONE: u32 = 0xD100_0400;
/// CBNZ X0 back one instruction (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CBNZ--Compare-and-branch-on-nonzero->).
const BRANCH_BACK_UNLESS_ZERO: u32 = 0xB5FF_FFE0;
/// BRK #0 (<https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BRK--Breakpoint-instruction->).
const BREAKPOINT: u32 = 0xD420_0000;
/// MRS X3, CNTVCT_EL0 (<https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CNTVCT-EL0--Counter-timer-Virtual-Count-Register>).
const READ_COUNTER: u32 = 0xD53B_E043;
/// MRS X3, CNTFRQ_EL0 (<https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CNTFRQ-EL0--Counter-timer-Frequency-Register>).
const READ_FREQUENCY: u32 = 0xD53B_E003;
/// The register the counter reads name.
const COUNTER_TARGET: u8 = 3;
/// CNTVCT_EL0's and CNTFRQ_EL0's encodings: op0, op1, CRn and CRm, then op2 of each.
const COUNTER_ENCODING: (u8, u8, u8, u8) = (3, 3, 14, 0);
const VIRTUAL_COUNT_OP2: u8 = 2;
const FREQUENCY_OP2: u8 = 0;
/// How many times the guest counts down, each time one progress event (its branch back).
const COUNT: u64 = 1 << 22;
/// The slice the bounded runs take: a small part of the count.
const SLICE: NonZeroU64 = NonZeroU64::new(1 << 16).expect("a slice");
/// How far a slice's end may lie from where the count says, in events: a stop is taken at the stop check after it.
const SLICE_SLACK: u64 = 1;

/// Counts interrupts, answering each continue or terminate; any other trap fails the test.
struct Interrupts {
    count: u64,
    terminate: bool,
}

impl TrapHandler for Interrupts {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse {
        panic!("a system call: {request:?}")
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        panic!("a memory fault at {address:#x} on {access:?}")
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.count += 1;
        if self.terminate {
            InterruptResponse::Terminate
        } else {
            InterruptResponse::Continue
        }
    }
}

/// Places the code at the lowest address of a new space of the executor, runs it from there with x0 holding `count`,
/// and returns the state it ended in and the interrupts it asked about.
fn run(
    executor: &mut WasmtimeExecutor,
    code: &[u32],
    count: u64,
    terminate: bool,
) -> (Processor<()>, u64) {
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
    let mut start = Processor {
        program_counter: LOWEST_ADDRESS,
        ..ZERO_PROCESSOR
    };
    start.general_purpose_registers[0] = count;
    let mut handler = Interrupts {
        count: 0,
        terminate,
    };
    let (execution, _interrupter) = executor.prepare_execution(start, &memory);
    let ExecutionStop::Terminated(stopped) = execution.run(&mut handler) else {
        panic!("the executor waited");
    };
    (stopped, handler.count)
}

/// Returns an executor whose runs take the slice given.
fn executor(slice: Option<NonZeroU64>) -> WasmtimeExecutor {
    WasmtimeExecutor::new(WasmtimeTranslatorConfig {
        slice,
        ..WasmtimeTranslatorConfig::default()
    })
}

const COUNT_DOWN: [u32; 3] = [SUBTRACT_ONE, BRANCH_BACK_UNLESS_ZERO, BREAKPOINT];

#[test]
fn slices_answered_continue_change_nothing() {
    let (stopped, interrupts) = run(&mut executor(Some(SLICE)), &COUNT_DOWN, COUNT, false);
    assert_eq!(
        (stopped.general_purpose_registers[0], stopped.exception),
        (0, Some(Exception::Breakpoint { comment: 0 })),
        "the state the count ended in"
    );
    let slices = COUNT / SLICE.get();
    assert!(
        interrupts.abs_diff(slices) <= SLICE_SLACK,
        "{interrupts} interrupts for {slices} slices"
    );
}

#[test]
fn a_slice_answered_terminate_ends_the_run_at_its_end() {
    let (stopped, interrupts) = run(&mut executor(Some(SLICE)), &COUNT_DOWN, COUNT, true);
    assert_eq!(interrupts, 1, "the interrupts the handler was given");
    assert_eq!(
        stopped.exception, None,
        "the exception of an interrupted run"
    );
    let counted = COUNT - stopped.general_purpose_registers[0];
    assert!(
        counted.abs_diff(SLICE.get()) <= SLICE_SLACK,
        "{counted} branches back run in a slice of {SLICE}"
    );
}

#[test]
fn no_slice_asks_nothing() {
    let (stopped, interrupts) = run(&mut executor(None), &COUNT_DOWN, COUNT, true);
    assert_eq!(
        (stopped.general_purpose_registers[0], interrupts),
        (0, 0),
        "the count and the interrupts"
    );
}

#[test]
fn counter_reads_trap_at_their_instruction() {
    for (word, op2) in [
        (READ_COUNTER, VIRTUAL_COUNT_OP2),
        (READ_FREQUENCY, FREQUENCY_OP2),
    ] {
        let (stopped, _) = run(&mut executor(None), &[word, BREAKPOINT], u64::MIN, false);
        let (op0, op1, crn, crm) = COUNTER_ENCODING;
        let trapped = SystemAccess {
            op0,
            op1,
            crn,
            crm,
            op2,
            rt: COUNTER_TARGET,
            direction: AccessDirection::Read,
        };
        assert_eq!(
            (stopped.program_counter, stopped.exception),
            (
                LOWEST_ADDRESS,
                Some(Exception::SystemAccessTrapped(trapped))
            ),
            "where the read of {word:#x} stopped"
        );
    }
}
