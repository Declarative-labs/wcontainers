use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
use wcontainers_processor_architecture_x86_64::X86_64;

wcontainers_distinguishing_state_generator_conformance::tests!(
    X86_64,
    CriticalValueCut::new(X86_64InputDomain)
);
