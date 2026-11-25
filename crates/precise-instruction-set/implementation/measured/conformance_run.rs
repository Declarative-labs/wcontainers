use std::convert::Infallible;
use std::path::Path;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, PreciseInstruction, User};

/// What a Neoverse V2 under KVM did, written by the record tool.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/neoverse-v2"
);

type Measured = MeasuredInstructionSet<Arm64<User>>;

fn recording() -> Recording {
    match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

/// Measures every instruction on the recording with the ARM64 state generator.
fn measure_recording() -> Vec<(Instruction<Infallible>, PreciseInstruction<Infallible>)> {
    match recording().measure_instructions(&CriticalValueCut::default()) {
        Ok(measurements) => measurements,
        Err(_) => panic!("the recording shows one known behavior of every instruction"),
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

wcontainers_precise_instruction_set_conformance::tests!(
    Measured,
    measure_recording(),
    recorded_cases()
);
