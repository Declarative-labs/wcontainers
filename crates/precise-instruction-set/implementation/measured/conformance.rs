use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_simulation::Simulation;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::Arm64;
use wcontainers_processor_architecture_recording::{Recording, record};

/// The set measured on a stand-in that plays the Neoverse V2.
fn measured() -> MeasuredInstructionSet<Arm64> {
    let known: [&dyn PreciseInstructionSet<Arm64>; 2] = [&AppleM5, &NeoverseV2];
    let measured = MeasuredInstructionSet::measure(
        &mut Simulation::new(NeoverseV2),
        &known,
        &CriticalValueProduct,
    );
    let Ok(measured) = measured else {
        panic!("the stand-in plays a known chip, so measuring it succeeds");
    };
    measured
}

fn recordings() -> Vec<Recording<Arm64>> {
    Arm64::list_instructions()
        .map(|instruction| {
            record(
                &mut Simulation::new(NeoverseV2),
                &CriticalValueProduct,
                instruction,
            )
        })
        .collect()
}

wcontainers_precise_instruction_set_conformance::tests!(measured(), recordings());
