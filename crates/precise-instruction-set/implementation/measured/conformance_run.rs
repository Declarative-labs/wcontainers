use std::convert::Infallible;
use std::path::Path;

use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, User};

/// What a Neoverse V2 under KVM did, written by the record tool.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/neoverse-v2"
);

type Measured = MeasuredInstructionSet<Recording, CriticalValueCut>;

fn recording() -> Recording {
    match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

/// An instruction, a state before and the state the chip gave after.
type RecordedCase = (
    Instruction<Infallible>,
    ExecutionState<'static, Arm64<User>>,
    ExecutionState<'static, Arm64<User>>,
);

fn recorded_cases() -> Vec<RecordedCase> {
    recording()
        .cases()
        .map(|(instruction, before, after)| (instruction, before.clone(), after.clone()))
        .collect()
}

wcontainers_precise_instruction_set_conformance::tests!(Measured, recording(), recorded_cases());
