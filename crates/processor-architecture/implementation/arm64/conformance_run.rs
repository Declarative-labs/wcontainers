use std::convert::Infallible;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{Arm64, PreciseInstruction, User};

/// The states of an instruction the suite takes, spread evenly over all the generator makes for it.
const STATES_PER_INSTRUCTION: usize = 1 << 9;

/// Each behavior of each instruction on states the ARM64 state generator makes for it.
fn cases() -> impl Iterator<
    Item = (
        PreciseInstruction<Infallible>,
        ExecutionState<'static, Arm64<User>>,
    ),
> {
    let generator = CriticalValueCut::default();
    Arm64::<User>::list_instructions().flat_map(move |instruction| {
        let behaviors: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
        let states: Vec<_> = if behaviors.is_empty() {
            Vec::new()
        } else {
            let all: Vec<_> = generator.generate_states(instruction).collect();
            let step = (all.len() / STATES_PER_INSTRUCTION).max(1);
            all.into_iter().step_by(step).collect()
        };
        behaviors.into_iter().flat_map(move |precise| {
            states
                .clone()
                .into_iter()
                .map(move |state| (precise, state))
        })
    })
}

wcontainers_processor_architecture_conformance::tests!(Arm64<User>, cases());
