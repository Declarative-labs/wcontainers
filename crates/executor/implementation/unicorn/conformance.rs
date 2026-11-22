//! The suite against unicorn's own set, and the instructions each chip's set is refused for.
//!
//! Every instruction a chip's set is not refused for has the same behavior in unicorn's set, so the suite's
//! run shows the stand-in agrees with that chip on it.

use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_unicorn::standin::StandIn;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
use wcontainers_precise_instruction_set_unicorn::UnicornMax;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction};

wcontainers_executor_conformance::tests!(
    StandIn::for_precise_set(&UnicornMax).expect("unicorn's own set is honored"),
    UnicornMax,
    CriticalValueProduct::new(Arm64InputDomain)
);

fn list_refused(set: &impl PreciseInstructionSet<Arm64>) -> Vec<Instruction> {
    match StandIn::for_precise_set(set) {
        Ok(_) => Vec::new(),
        Err(refusals) => refusals
            .iter()
            .map(|refusal| refusal.requested.to_instruction())
            .collect(),
    }
}

#[test]
fn refuses_the_apple_m5_set_where_unicorn_differs() {
    assert_eq!(
        list_refused(&AppleM5),
        [Instruction::LdrImmediatePostIndex64, Instruction::MrsCtrEl0]
    );
}

#[test]
fn refuses_the_neoverse_v2_set_where_unicorn_differs() {
    assert_eq!(
        list_refused(&NeoverseV2),
        [
            Instruction::LdrImmediatePostIndex64,
            Instruction::Rdvl,
            Instruction::MrsCtrEl0
        ]
    );
}
