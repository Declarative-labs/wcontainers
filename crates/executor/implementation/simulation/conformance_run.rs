use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_simulation::Simulation;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set_apple_m5::AppleM5;

wcontainers_executor_conformance::tests!(
    Simulation::new(AppleM5),
    AppleM5,
    CriticalValueProduct::new(Arm64InputDomain)
);
