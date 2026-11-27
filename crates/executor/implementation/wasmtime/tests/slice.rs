//! The ten instructions of the first slice over every state the generator makes for them: the executor shows one known
//! behavior of each, and that behavior keeps the choices the translator was written to make.

use std::convert::Infallible;
use std::time::Instant;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_wasmtime::{WasmtimeError, WasmtimeExecutor, WasmtimeTranslatorConfig};
use wcontainers_precise_instruction_set_translator::{SLICE, keeps_translator_choices};
use wcontainers_processor_architecture::{
    ExecutionState, ExecutionStateExpectation, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_arm64::{Arm64, Exception, Instruction, User};

/// What one instruction's states gave.
#[derive(Debug, Default)]
struct Outcome {
    states: usize,
    executed: usize,
    /// States reaching past the 4 GiB of addresses a space has, which the executor refuses.
    refused: usize,
    /// States on which the executor did what no behavior still in play does.
    disagreeing: usize,
    /// Of those, the states that agree with a behavior once the fault address gets the tag bits that behavior keeps.
    fault_address_tag_only: usize,
}

/// The bits of an address below its tag, the top byte that Top Byte Ignore leaves out.
const UNTAGGED_BITS: u64 = (1 << 56) - 1;

/// Returns whether the executed state meets the expectation once its fault address takes the expected tag bits.
fn differs_only_in_fault_address_tag(
    executed: &ExecutionState<'_, Arm64<User>>,
    expected: &ExecutionStateExpectation<'_, Arm64<User>>,
) -> bool {
    let (
        Some(Exception::DataAbort {
            status,
            access,
            address,
        }),
        Some(Exception::DataAbort {
            address: tagged, ..
        }),
    ) = (
        executed.processor.exception,
        expected.processor.processor.exception,
    )
    else {
        return false;
    };
    if tagged == address || tagged & UNTAGGED_BITS != address {
        return false;
    }
    let mut retagged = executed.clone();
    retagged.processor.exception = Some(Exception::DataAbort {
        status,
        access,
        address: tagged,
    });
    retagged.meets_expectations(expected)
}

/// Runs every state of the instruction and returns its outcome and the behaviors every executed state met.
fn measure(
    executor: &mut WasmtimeExecutor,
    generator: &CriticalValueCut,
    instruction: Instruction<Infallible>,
) -> (
    Outcome,
    Vec<<Arm64<User> as ProcessorArchitecture>::PreciseInstruction>,
) {
    let mut candidates: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
    let mut outcome = Outcome::default();
    for before in generator.generate_states(instruction) {
        outcome.states += 1;
        let executed = match executor.execute_single_instruction(before.clone()) {
            Ok(executed) => executed,
            Err(WasmtimeError::AddressOutsideSpace) => {
                outcome.refused += 1;
                continue;
            }
            Err(error) => panic!("{instruction:?}: {error}"),
        };
        outcome.executed += 1;
        let kept: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|precise| {
                let Ok(expected) = precise.simulate_state(before.clone());
                executed.meets_expectations(&expected)
            })
            .collect();
        if kept.is_empty() {
            outcome.disagreeing += 1;
            let tag_only = candidates.iter().any(|precise| {
                let Ok(expected) = precise.simulate_state(before.clone());
                differs_only_in_fault_address_tag(&executed, &expected)
            });
            if tag_only {
                outcome.fault_address_tag_only += 1;
            }
            if outcome.disagreeing == 1 {
                eprintln!(
                    "{instruction:?}: no behavior agrees on\n  before: {:?}\n  executed: {:?}",
                    before.processor, executed.processor.exception
                );
            }
        } else {
            candidates = kept;
        }
    }
    (outcome, candidates)
}

#[test]
fn slice_instructions_agree_over_the_generated_states() {
    let generator = CriticalValueCut::default();
    let mut executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
    let mut failures = Vec::new();
    let (mut states, mut executed, mut refused) = (0, 0, 0);
    let started = Instant::now();
    for &instruction in SLICE {
        let (outcome, behaviors) = measure(&mut executor, &generator, instruction);
        eprintln!(
            "{instruction:?}: {} states, {} executed, {} refused, {} disagree ({} only in the fault address's tag); \
             behaviors left: {behaviors:?}",
            outcome.states,
            outcome.executed,
            outcome.refused,
            outcome.disagreeing,
            outcome.fault_address_tag_only
        );
        states += outcome.states;
        executed += outcome.executed;
        refused += outcome.refused;
        let one_keeping = behaviors.len() == 1 && behaviors.iter().all(keeps_translator_choices);
        if outcome.disagreeing > 0 || !one_keeping {
            failures.push(format!(
                "{instruction:?}: {outcome:?}, {} behaviors left",
                behaviors.len()
            ));
        }
    }
    eprintln!(
        "{states} states, {executed} executed, {refused} refused, in {:?}",
        started.elapsed()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
