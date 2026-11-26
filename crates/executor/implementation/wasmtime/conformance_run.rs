use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_wasmtime::{Arm64Executor, TranslatorConfig};
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set_translator::Translator;

wcontainers_executor_conformance::tests!(
    Arm64Executor::new(TranslatorConfig::default()),
    Translator,
    CriticalValueProduct::new(Arm64InputDomain)
);
