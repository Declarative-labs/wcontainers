use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_processor_architecture_arm64::{Arm64, User};

wcontainers_distinguishing_state_generator_conformance::tests!(
    Arm64<User>,
    CriticalValueCut::default()
);
