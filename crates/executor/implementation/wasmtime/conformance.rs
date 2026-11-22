//! The suite against the translator's own set, and the instructions each chip's set is refused for.
//!
//! Every instruction a chip's set is not refused for has the same behavior in the translator's set, so the
//! suite's run shows the executor agrees with that chip on it.

use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_wasmtime::{Arm64Executor, TranslatorConfig};
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
use wcontainers_precise_instruction_set_translator::Translator;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction};

wcontainers_executor_conformance::tests!(
    Arm64Executor::for_precise_set(&Translator, TranslatorConfig::default())
        .expect("the translator's own set is honored"),
    Translator,
    CriticalValueProduct::new(Arm64InputDomain)
);

fn list_refused(set: &impl PreciseInstructionSet<Arm64>) -> Vec<Instruction> {
    match Arm64Executor::for_precise_set(set, TranslatorConfig::default()) {
        Ok(_) => Vec::new(),
        Err(refusals) => refusals
            .iter()
            .map(|refusal| refusal.requested.to_instruction())
            .collect(),
    }
}

#[test]
fn refuses_the_apple_m5_set_where_the_translator_differs() {
    assert_eq!(
        list_refused(&AppleM5),
        [Instruction::LdrImmediatePostIndex64, Instruction::MrsCtrEl0]
    );
}

#[test]
fn refuses_the_neoverse_v2_set_where_the_translator_differs() {
    assert_eq!(
        list_refused(&NeoverseV2),
        [
            Instruction::LdrImmediatePostIndex64,
            Instruction::Rdvl,
            Instruction::MrsCtrEl0
        ]
    );
}
