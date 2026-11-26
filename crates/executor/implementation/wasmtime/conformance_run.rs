use std::borrow::Cow;

use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_conformance::AGREEMENT_EXECUTORS;
use wcontainers_executor_wasmtime::{WasmtimeExecutor, WasmtimeTranslatorConfig};
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, InstructionFetch, PagePermission, Processor, User, ZERO_PROCESSOR,
};

/// Returns for every instruction the first known behavior that each state the executor executes meets, or its first
/// known behavior where none does: the translator's set, measured.
///
/// The instructions are shared out among as many executors as the agreement test runs.
fn list_translator_behaviors() -> MeasuredInstructionSet<Arm64<User>> {
    let instructions: Vec<_> = Arm64::<User>::list_instructions().collect();
    let mut measurements: Vec<_> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..AGREEMENT_EXECUTORS)
            .map(|share| {
                let instructions = &instructions;
                scope.spawn(move || {
                    let generator = CriticalValueCut::default();
                    let mut executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
                    instructions
                        .iter()
                        .enumerate()
                        .skip(share)
                        .step_by(AGREEMENT_EXECUTORS)
                        .filter_map(|(index, instruction)| {
                            let shown =
                                find_shown_behavior(&mut executor, &generator, *instruction)?;
                            Some((index, (*instruction, shown)))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        runs.into_iter()
            .flat_map(|run| run.join().expect("a measuring run ends"))
            .collect()
    });
    measurements.sort_by_key(|(index, _)| *index);
    let measurements: Vec<_> = measurements
        .into_iter()
        .map(|(_, measurement)| measurement)
        .collect();
    MeasuredInstructionSet::from_measurements(&measurements)
}

/// Returns the first known behavior of the instruction every state the executor executes meets, or its first known
/// behavior where none does; none where it has no known behavior.
fn find_shown_behavior(
    executor: &mut WasmtimeExecutor,
    generator: &CriticalValueCut,
    instruction: <Arm64<User> as ProcessorArchitecture>::Instruction,
) -> Option<<Arm64<User> as ProcessorArchitecture>::PreciseInstruction> {
    let known: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
    let first = *known.first()?;
    let executed: Vec<_> = generator
        .generate_states(instruction)
        .filter_map(|before| {
            let after = executor.execute_single_instruction(before.clone()).ok()?;
            Some((before, after))
        })
        .collect();
    let shown = known.into_iter().find(|precise| {
        executed.iter().all(|(before, after)| {
            let Ok(expected) = precise.simulate_state(before.clone());
            after.meets_expectations(&expected)
        })
    });
    Some(shown.unwrap_or(first))
}

/// The lowest address past a space's 4 GiB of addresses.
const PAST_THE_SPACE: u64 = 1 << u32::BITS;
/// BRK #0, the word the state the executor refuses runs.
const BREAKPOINT_WORD: u32 = 0xD420_0000;

/// The states the executor refuses: one whose instruction lies past the addresses a space has.
fn list_unexecutable_states() -> Vec<ExecutionState<'static, Arm64<User>>> {
    let at = PAST_THE_SPACE + LOWEST_ADDRESS;
    vec![ExecutionState {
        processor: Processor {
            program_counter: at,
            ..ZERO_PROCESSOR
        },
        memory_mutations: vec![
            MemoryDelta::PermissionAssignment {
                addresses: at..at + PAGE_SIZE,
                permission: PagePermission::Mapped {
                    data_access: DataAccess::ReadOnly,
                    instruction_fetch: InstructionFetch::Allowed,
                },
            },
            MemoryDelta::ValueAssignment {
                start: at,
                bytes: Cow::Owned(BREAKPOINT_WORD.to_le_bytes().to_vec()),
            },
        ],
    }]
}

// The suite counts the interrupts another thread raises, so its runs take no slice of their own; tests/time_slice.rs
// checks the slice.
wcontainers_executor_conformance::tests!(
    WasmtimeExecutor::new(WasmtimeTranslatorConfig {
        slice: None,
        ..WasmtimeTranslatorConfig::default()
    }),
    list_translator_behaviors(),
    CriticalValueCut::default(),
    list_unexecutable_states()
);
