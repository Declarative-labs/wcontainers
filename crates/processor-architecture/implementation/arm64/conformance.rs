use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{Arm64, PreciseInstruction, list_behaviors};

const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
    CriticalValueProduct::new(Arm64InputDomain);

/// Each behavior of each instruction on every state the critical value generator makes for it.
fn cases() -> impl Iterator<Item = (PreciseInstruction, ExecutionState<'static, Arm64>)> {
    Arm64::list_instructions()
        .flat_map(list_behaviors)
        .flat_map(|precise| {
            DistinguishingStateGenerator::<Arm64>::generate_states(
                &GENERATOR,
                precise.to_instruction(),
            )
            .map(move |state| (precise, state))
        })
}

wcontainers_processor_architecture_conformance::tests!(cases());
