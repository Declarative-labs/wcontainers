use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_processor_architecture_arm64::{Arm64, User};

wcontainers_distinguishing_state_generator_conformance::tests!(
    Arm64<User>,
    CriticalValueProduct::new(Arm64InputDomain)
);
