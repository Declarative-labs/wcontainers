use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_accumulator_machine::AccumulatorMachineInstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_accumulator_machine::{
    AccumulatorMachineProcessorArchitecture, PreciseInstruction,
};

const GENERATOR: CriticalValueProduct<AccumulatorMachineInstructionInputDomain> =
    CriticalValueProduct::new(AccumulatorMachineInstructionInputDomain);

/// Returns each behavior of each instruction on every state the critical value generator makes for it.
fn cases() -> impl Iterator<
    Item = (
        PreciseInstruction,
        ExecutionState<'static, AccumulatorMachineProcessorArchitecture>,
    ),
> {
    AccumulatorMachineProcessorArchitecture::list_instructions()
        .flat_map(AccumulatorMachineProcessorArchitecture::list_known_behaviors)
        .flat_map(|precise| {
            DistinguishingStateGenerator::<AccumulatorMachineProcessorArchitecture>::generate_states(
                &GENERATOR,
                precise.to_instruction(),
            )
            .map(move |state| (precise, state))
        })
}

wcontainers_processor_architecture_conformance::tests!(
    AccumulatorMachineProcessorArchitecture,
    cases()
);
