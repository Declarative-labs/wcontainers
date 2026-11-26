use std::borrow::Cow;

use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_conformance::AGREEMENT_EXECUTORS;
use wcontainers_executor_unicorn::standin::StandIn;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, InstructionFetch, PagePermission, Processor, User, ZERO_PROCESSOR,
};

/// Returns for every instruction the first known behavior that each state the stand-in executes meets, or its first
/// known behavior where none does.
///
/// Where the architecture knows several behaviors of an instruction, the chip picks one, and the stand-in is the chip
/// QEMU's `max` model is: the agreement test then fails exactly where no known behavior explains what it does.
///
/// The instructions are shared out among as many stand-ins as the agreement test runs.
fn list_stand_in_behaviors() -> MeasuredInstructionSet<Arm64<User>> {
    let instructions: Vec<_> = Arm64::<User>::list_instructions().collect();
    let mut measurements: Vec<_> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..AGREEMENT_EXECUTORS)
            .map(|share| {
                let instructions = &instructions;
                scope.spawn(move || {
                    let generator = CriticalValueCut::default();
                    let mut stand_in = StandIn::new();
                    instructions
                        .iter()
                        .enumerate()
                        .skip(share)
                        .step_by(AGREEMENT_EXECUTORS)
                        .filter_map(|(index, instruction)| {
                            let shown =
                                find_shown_behavior(&mut stand_in, &generator, *instruction)?;
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

/// Returns the first known behavior of the instruction every state the stand-in executes meets, or its first known
/// behavior where none does; none where it has no known behavior.
fn find_shown_behavior(
    stand_in: &mut StandIn,
    generator: &CriticalValueCut,
    instruction: <Arm64<User> as ProcessorArchitecture>::Instruction,
) -> Option<<Arm64<User> as ProcessorArchitecture>::PreciseInstruction> {
    let known: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
    let first = *known.first()?;
    let executed: Vec<_> = generator
        .generate_states(instruction)
        .filter_map(|before| {
            let after = stand_in.execute_single_instruction(before.clone()).ok()?;
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

/// An unallocated Advanced SIMD three same (FP16) word, on which QEMU's decoder asserts.
const EMULATOR_ABORTING_WORD: u32 = 0x0EC0_1DCD;
/// MRS X0, DAIF, which EL1 traps at EL0 and unicorn stops on as undefined.
const TRAPPED_WORD: u32 = 0xD53B_4220;

/// The states the stand-in refuses: one whose instruction would end the host process, one whose stop it cannot tell.
fn list_unexecutable_states() -> Vec<ExecutionState<'static, Arm64<User>>> {
    [EMULATOR_ABORTING_WORD, TRAPPED_WORD]
        .map(|word| ExecutionState {
            processor: Processor {
                program_counter: LOWEST_ADDRESS,
                ..ZERO_PROCESSOR
            },
            memory_mutations: vec![
                MemoryDelta::PermissionAssignment {
                    addresses: LOWEST_ADDRESS..LOWEST_ADDRESS + PAGE_SIZE,
                    permission: PagePermission::Mapped {
                        data_access: DataAccess::ReadOnly,
                        instruction_fetch: InstructionFetch::Allowed,
                    },
                },
                MemoryDelta::ValueAssignment {
                    start: LOWEST_ADDRESS,
                    bytes: Cow::Owned(word.to_le_bytes().to_vec()),
                },
            ],
        })
        .into()
}

wcontainers_executor_conformance::tests!(
    StandIn::new(),
    list_stand_in_behaviors(),
    CriticalValueCut::default(),
    list_unexecutable_states()
);
