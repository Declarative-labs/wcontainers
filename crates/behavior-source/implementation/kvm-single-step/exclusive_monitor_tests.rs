//! The one-sided check of the exclusive monitor on the live chip, outside the probe: a chip may lose the monitor for
//! reasons no host sees, so this asserts only what was measured to hold every time
//! (`hidden-state-loading/ORCHESTRATOR-RULING.md`, ruling 3).
//!
//! The runs leave the step and the breakpoint off, so a load exclusive and a store exclusive run in one entry at EL0.

use std::borrow::Cow;

use wcontainers_abi::PAGE_SIZE;
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, Exception, INSTRUCTION_SIZE, User,
};

use crate::machine::Register;
use crate::tests::{BREAKPOINT, CODE, DATA, NO_OPERATION, to_machine, to_state};
use crate::{KvmSingleStepMachine, MemoryLoading};

/// [LDXR XZR, \[X1\]](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDXR--Load-exclusive-register-),
/// which marks the granule holding X1 and writes no register.
const LOAD_EXCLUSIVE: u32 = 0xC85F_7C3F;
/// [STXR W2, X3, \[X4\]](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STXR--Store-exclusive-register-).
const STORE_EXCLUSIVE: u32 = 0xC802_7C83;
/// [MRS X0, CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register), which EL0 may read here.
const READ_CACHE_TYPE: u32 = 0xD53B_0020;
/// [ERG](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register#:~:text=ERG%2C%20bits%20%5B23%3A20%5D):
/// log2 of the reservation granule in words.
const GRANULE_SHIFT: u64 = 20;
const GRANULE_BITS: u64 = 0b1111;
const WORD_BYTES: u64 = size_of::<u32>() as u64;
const STORED_BYTES: u64 = size_of::<u64>() as u64;
/// What the store exclusive writes, a value with no zero byte, and what W2 holds before it.
const STORED: u64 = 0x0123_4567_89AB_CDEF;
const STATUS_BEFORE: u64 = 0xFF;
/// What the status register holds after a store that happened and after one that did not.
const STORE_HAPPENED: u64 = 0;
const STORE_FAILED: u64 = 1;
/// Runs of each case: the chip stored after its load exclusive in at least 99.9% of runs (MEASURED, ruling).
const TRIES: usize = 200;

/// The state about to run the words from [`CODE`], with X1 the address loaded and X4 the address stored.
fn to_sequence(words: &[u32], loaded: u64, stored: u64) -> ExecutionState<'static, Arm64<User>> {
    let mut state = to_state(words[0], loaded, ConditionFlags::empty());
    let registers = &mut state.processor.general_purpose_registers;
    registers[2] = STATUS_BEFORE;
    registers[3] = STORED;
    registers[4] = stored;
    state.memory_mutations[1] = MemoryDelta::ValueAssignment {
        start: CODE,
        bytes: Cow::Owned(words.iter().flat_map(|word| word.to_le_bytes()).collect()),
    };
    state
}

/// A machine on which a run goes on until an exception, with neither the step nor the breakpoint on.
fn to_unstepped_machine() -> KvmSingleStepMachine {
    let mut chip = to_machine(MemoryLoading::Incremental);
    for register in [Register::DebugControl, Register::BreakpointControl] {
        if let Err(error) = chip.machine.set(register, 0) {
            panic!("turning off {register:?}: {error:?}");
        }
    }
    chip
}

/// Runs the words, which end in a BRK, and gives the state after.
fn run_sequence(
    chip: &mut KvmSingleStepMachine,
    before: &ExecutionState<'static, Arm64<User>>,
    words: usize,
) -> ExecutionState<'static, Arm64<User>> {
    let after = match chip.execute_single_instruction(before.clone()) {
        Ok(after) => after,
        Err(refusal) => panic!("the chip refused the sequence: {refusal:?}"),
    };
    assert_eq!(
        (after.processor.exception, after.processor.program_counter),
        (
            Some(Exception::Breakpoint { comment: 0 }),
            CODE + (words as u64 - 1) * INSTRUCTION_SIZE
        )
    );
    after
}

/// Whether the store exclusive stored, after checking that memory and the status register tell the same: the value
/// at the address and status 0, or memory untouched and status 1.
fn is_stored(
    chip: &mut KvmSingleStepMachine,
    before: &ExecutionState<'static, Arm64<User>>,
) -> bool {
    let address = before.processor.general_purpose_registers[4];
    let after = run_sequence(chip, before, STORE_SEQUENCE_WORDS);
    let changes: Vec<_> = after.memory_mutations[before.memory_mutations.len()..]
        .iter()
        .map(|change| match change {
            MemoryDelta::ValueAssignment { start, bytes } => (*start, bytes.to_vec()),
            MemoryDelta::PermissionAssignment { .. } => panic!("the chip changed a permission"),
        })
        .collect();
    match after.processor.general_purpose_registers[2] {
        STORE_HAPPENED => {
            assert_eq!(
                changes,
                [(address, STORED.to_le_bytes().to_vec())],
                "{address:#x}"
            );
            true
        }
        STORE_FAILED => {
            assert_eq!(changes, [], "{address:#x}");
            false
        }
        other => panic!("{address:#x}: status {other:#x}"),
    }
}

/// The words of a store sequence: a load exclusive or a NOP, the store exclusive and a BRK.
const STORE_SEQUENCE_WORDS: usize = 3;

/// Counts the runs of [`TRIES`] whose store exclusive stored.
fn count_stores(chip: &mut KvmSingleStepMachine, first: u32, loaded: u64, stored: u64) -> usize {
    let before = to_sequence(&[first, STORE_EXCLUSIVE, BREAKPOINT], loaded, stored);
    (0..TRIES).filter(|_| is_stored(chip, &before)).count()
}

/// Without a load exclusive the store never happens; after one, a store inside the reservation granule CTR_EL0 gives
/// happens with status 0 or not with status 1, and at least once; outside that granule it never happens.
#[test]
#[ignore = "needs KVM"]
fn a_store_exclusive_stores_only_inside_the_granule_a_load_exclusive_marked() {
    let mut chip = to_unstepped_machine();
    let cache_type = run_sequence(
        &mut chip,
        &to_sequence(&[READ_CACHE_TYPE, BREAKPOINT], DATA, DATA),
        2,
    )
    .processor
    .general_purpose_registers[0];
    let granule = WORD_BYTES << (cache_type >> GRANULE_SHIFT & GRANULE_BITS);
    let marked = DATA + PAGE_SIZE / 2;
    assert!(marked.is_multiple_of(granule), "granule {granule}");
    let last_in_granule = marked + granule - STORED_BYTES;
    let counts = [
        count_stores(&mut chip, NO_OPERATION, marked, marked),
        count_stores(&mut chip, LOAD_EXCLUSIVE, marked, marked),
        count_stores(&mut chip, LOAD_EXCLUSIVE, marked, last_in_granule),
        count_stores(&mut chip, LOAD_EXCLUSIVE, marked, marked + granule),
    ];
    println!("granule {granule} bytes; stores of {TRIES}: {counts:?}");
    let [unpaired, paired, inside, outside] = counts;
    assert_eq!((unpaired, outside), (0, 0));
    assert!(paired > 0 && inside > 0);
}
