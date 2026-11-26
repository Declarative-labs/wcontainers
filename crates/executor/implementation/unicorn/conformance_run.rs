use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_unicorn::standin::StandIn;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{Arm64, User};

/// Returns the first known behavior of every instruction that has one.
fn list_first_behaviors() -> MeasuredInstructionSet<Arm64<User>> {
    let first: Vec<_> = Arm64::<User>::list_instructions()
        .filter_map(|instruction| {
            Arm64::<User>::list_known_behaviors(instruction)
                .next()
                .map(|precise| (instruction, precise))
        })
        .collect();
    MeasuredInstructionSet::from_measurements(&first)
}

/// The states unicorn stops on for a reason the stand-in cannot give as the processor's: none known.
fn list_unexecutable_states() -> Vec<ExecutionState<'static, Arm64<User>>> {
    Vec::new()
}

wcontainers_executor_conformance::tests!(
    StandIn::new(),
    list_first_behaviors(),
    CriticalValueCut::default(),
    list_unexecutable_states()
);
