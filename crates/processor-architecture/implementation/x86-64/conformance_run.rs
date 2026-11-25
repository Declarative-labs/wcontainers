use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::{PreciseInstruction, X86_64};

const GENERATOR: CriticalValueProduct<X86_64InputDomain> =
    CriticalValueProduct::new(X86_64InputDomain);

/// Each behavior of each form on every state the critical value generator makes for it.
fn cases() -> impl Iterator<Item = (PreciseInstruction, ExecutionState<'static, X86_64>)> {
    X86_64::list_instructions()
        .flat_map(X86_64::list_known_behaviors)
        .flat_map(|precise| {
            DistinguishingStateGenerator::<X86_64>::generate_states(
                &GENERATOR,
                precise.to_instruction(),
            )
            .map(move |state| (precise, state))
        })
}

wcontainers_processor_architecture_conformance::tests!(X86_64, cases());
