use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor::Executor;
use wcontainers_executor_simulation::Simulation;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{Arm64, Instruction};

const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
    CriticalValueProduct::new(Arm64InputDomain);

/// The set measured on a stand-in that plays the Neoverse V2.
fn measured() -> MeasuredInstructionSet<Arm64> {
    let known: [&dyn PreciseInstructionSet<Arm64>; 2] = [&AppleM5, &NeoverseV2];
    match MeasuredInstructionSet::measure(&mut Simulation::new(NeoverseV2), &known, &GENERATOR) {
        Ok(measured) => measured,
        Err(failure) => panic!("the stand-in plays a known chip: {failure:?}"),
    }
}

/// What the stand-in playing the Neoverse V2 does on every generated state.
fn recorded_cases() -> Vec<(
    Instruction,
    ExecutionState<'static, Arm64>,
    ExecutionState<'static, Arm64>,
)> {
    let mut chip = Simulation::new(NeoverseV2);
    Arm64::list_instructions()
        .flat_map(|instruction| {
            DistinguishingStateGenerator::<Arm64>::generate_states(&GENERATOR, instruction)
                .map(move |before| (instruction, before))
                .collect::<Vec<_>>()
        })
        .map(|(instruction, before)| {
            let after = chip.execute_single_instruction(before.clone());
            (instruction, before, after)
        })
        .collect()
}

wcontainers_precise_instruction_set_conformance::tests!(measured(), recorded_cases());
