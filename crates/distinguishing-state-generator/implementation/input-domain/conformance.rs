use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::{
    Arm64, Instruction, PreciseInstruction, list_behaviors,
};

fn candidates() -> Vec<(Instruction, Vec<PreciseInstruction>)> {
    Arm64::list_instructions()
        .map(|instruction| (instruction, list_behaviors(instruction).collect()))
        .collect()
}

wcontainers_distinguishing_state_generator_conformance::tests!(
    Arm64,
    CriticalValueProduct::new(Arm64InputDomain),
    candidates()
);
