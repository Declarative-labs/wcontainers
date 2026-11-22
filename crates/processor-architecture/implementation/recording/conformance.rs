use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_simulation::Simulation;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction};
use wcontainers_processor_architecture_recording::{Recording, record};

/// Every combination of the critical values of each instruction's operands.
const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
    CriticalValueProduct::new(Arm64InputDomain);

/// The instructions with the fewest generated states, since each case carries its own copy of the recording.
const INSTRUCTIONS: [Instruction; 3] =
    [Instruction::Brk, Instruction::Rdvl, Instruction::MrsCtrEl0];

/// Each recorded state before, and one that no recording holds, with the recording of its instruction.
fn cases() -> impl Iterator<Item = (Recording<Arm64>, ExecutionState<'static, Arm64>)> {
    INSTRUCTIONS.into_iter().flat_map(|instruction| {
        let recording = record(&mut Simulation::new(AppleM5), &GENERATOR, instruction);
        let mut unrecorded = recording.cases()[0].0.clone();
        unrecorded.processor.program_counter += 4;
        let befores = recording.cases().iter().map(|(before, _)| before.clone());
        let befores: Vec<_> = befores.chain([unrecorded]).collect();
        befores
            .into_iter()
            .map(move |before| (recording.clone(), before))
    })
}

wcontainers_processor_architecture_conformance::tests!(cases());
