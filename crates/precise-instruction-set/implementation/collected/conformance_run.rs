use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_accumulator_machine::AccumulatorMachineInstructionInputDomain;
use wcontainers_precise_instruction_set_collected::CollectedPreciseInstructionSet;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_accumulator_machine::{
    AccumulatorMachineProcessorArchitecture, Instruction, PreciseInstruction,
};

const GENERATOR: CriticalValueProduct<AccumulatorMachineInstructionInputDomain> =
    CriticalValueProduct::new(AccumulatorMachineInstructionInputDomain);

/// Gives each instruction its last known behavior, so that no measurement is its first.
fn measure_last_behaviors() -> Vec<(Instruction, PreciseInstruction)> {
    AccumulatorMachineProcessorArchitecture::list_instructions()
        .filter_map(|instruction| {
            AccumulatorMachineProcessorArchitecture::list_known_behaviors(instruction)
                .last()
                .map(|precise| (instruction, precise))
        })
        .collect()
}

/// An instruction, a state before and the state its last known behavior leaves after.
type RecordedCase = (
    Instruction,
    ExecutionState<'static, AccumulatorMachineProcessorArchitecture>,
    ExecutionState<'static, AccumulatorMachineProcessorArchitecture>,
);

/// Records every generated state of each instruction as a core showing its last known behavior executes it.
fn record_last_behaviors() -> Vec<RecordedCase> {
    measure_last_behaviors()
        .into_iter()
        .flat_map(|(instruction, precise)| {
            DistinguishingStateGenerator::<AccumulatorMachineProcessorArchitecture>::generate_states(&GENERATOR, instruction)
                .map(move |before| {
                    let Ok(expectation) = precise.simulate_state(before.clone());
                    let after = ExecutionState {
                        processor: expectation.processor,
                        memory_mutations: expectation.memory_mutations,
                    };
                    (instruction, before, after)
                })
        })
        .collect()
}

wcontainers_precise_instruction_set_conformance::tests!(
    CollectedPreciseInstructionSet<AccumulatorMachineProcessorArchitecture>,
    measure_last_behaviors(),
    record_last_behaviors()
);
