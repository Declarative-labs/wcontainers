use std::path::Path;

use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, User};

/// What a Neoverse V2 under KVM did with every listed instruction, written by `record -- <folder> all`.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/recordings/neoverse-v2-whole-set"
);

/// The generator the recording was made from.
static GENERATOR: CriticalValueCut = CriticalValueCut::new(Arm64InputDomain);

fn recording() -> Recording {
    match Recording::read_generated(Path::new(RECORDINGS), &GENERATOR) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

/// Returns a state the recording holds no case of: its first state before, with another stack pointer.
fn to_unrecorded() -> Vec<ExecutionState<'static, Arm64<User>>> {
    recording()
        .generate_cases(&GENERATOR)
        .take(1)
        .map(|(_, mut unrecorded, _)| {
            unrecorded.processor.stack_pointer ^= 1;
            unrecorded
        })
        .collect()
}

wcontainers_behavior_source_conformance::tests!(recording(), GENERATOR, to_unrecorded());
