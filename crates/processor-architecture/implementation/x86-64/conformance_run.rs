use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::{PreciseInstruction, X86_64};

const GENERATOR: CriticalValueProduct<X86_64InputDomain> =
    CriticalValueProduct::new(X86_64InputDomain);

/// Each behavior of each form on every state the critical value generator makes for it, the states made once a form.
fn cases() -> impl Iterator<Item = (PreciseInstruction, ExecutionState<'static, X86_64>)> {
    X86_64::list_instructions().flat_map(|instruction| {
        let behaviors: Vec<PreciseInstruction> = X86_64::list_known_behaviors(instruction).collect();
        let states: Vec<ExecutionState<'static, X86_64>> = if behaviors.is_empty() {
            Vec::new()
        } else {
            DistinguishingStateGenerator::<X86_64>::generate_states(&GENERATOR, instruction).collect()
        };
        behaviors
            .into_iter()
            .flat_map(move |precise| states.clone().into_iter().map(move |state| (precise, state)))
    })
}

wcontainers_processor_architecture_conformance::tests!(X86_64, cases());
