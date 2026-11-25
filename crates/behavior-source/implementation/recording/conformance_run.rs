use std::path::Path;

use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, User};

/// What a Neoverse V2 under KVM did, written by the record tool.
const RECORDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recordings/neoverse-v2");

fn recording() -> Recording {
    match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

/// A state the recording holds no case of: its first state before, with another stack pointer.
fn to_unrecorded() -> Vec<ExecutionState<'static, Arm64<User>>> {
    recording()
        .cases()
        .take(1)
        .map(|(_, before, _)| {
            let mut unrecorded = before.clone();
            unrecorded.processor.stack_pointer ^= 1;
            unrecorded
        })
        .collect()
}

wcontainers_behavior_source_conformance::tests!(
    recording(),
    CriticalValueProduct::new(Arm64InputDomain),
    to_unrecorded()
);
