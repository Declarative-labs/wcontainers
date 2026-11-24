use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_unicorn::standin::StandIn;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set_unicorn::UnicornMax;

wcontainers_executor_conformance::tests!(
    StandIn::new(),
    UnicornMax,
    CriticalValueProduct::new(Arm64InputDomain)
);
