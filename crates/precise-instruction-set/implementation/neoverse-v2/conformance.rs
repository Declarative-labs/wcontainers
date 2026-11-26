use std::path::Path;

use wcontainers_behavior_source_recording::Recording;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, PreciseInstruction, User};

/// What a Neoverse V2 under KVM did, written by the record tool, in the recording source.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/neoverse-v2"
);

fn read_recording() -> Recording {
    match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

fn recorded_cases() -> Vec<(
    Instruction<core::convert::Infallible>,
    ExecutionState<'static, Arm64<User>>,
    ExecutionState<'static, Arm64<User>>,
)> {
    read_recording()
        .cases()
        .map(|(instruction, before, after)| (instruction, before.clone(), after.clone()))
        .collect()
}

/// The set's own choice for each recorded instruction, since its choices are written out, not measured.
fn written_choices() -> Vec<(
    Instruction<core::convert::Infallible>,
    PreciseInstruction<core::convert::Infallible>,
)> {
    read_recording()
        .instructions()
        .map(|instruction| (instruction, NeoverseV2.to_precise(instruction)))
        .collect()
}

wcontainers_precise_instruction_set_conformance::tests!(
    NeoverseV2,
    written_choices(),
    recorded_cases()
);
