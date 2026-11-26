use std::convert::Infallible;
use std::path::Path;
use std::sync::LazyLock;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, PreciseInstruction, User};

/// What a Neoverse V2 under KVM did with every listed instruction, written by the record tool.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/neoverse-v2-whole-set"
);

/// The generator the recording was made from.
static GENERATOR: CriticalValueCut = CriticalValueCut::new(Arm64InputDomain);

/// The recording read once, whose cases every test may walk.
static RECORDING: LazyLock<Recording> = LazyLock::new(recording);

type Measured = MeasuredInstructionSet<Arm64<User>>;

fn recording() -> Recording {
    match Recording::read_generated(Path::new(RECORDINGS), &GENERATOR) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

/// Measures every instruction on the recording with the ARM64 state generator.
fn measure_recording() -> Vec<(Instruction<Infallible>, PreciseInstruction<Infallible>)> {
    match recording().measure_instructions(&GENERATOR) {
        Ok(measurements) => measurements,
        Err(_) => panic!("the recording shows one known behavior of every instruction"),
    }
}

/// Each recorded instruction with a state before and the state the chip gave after.
fn recorded_cases() -> impl Iterator<
    Item = (
        Instruction<Infallible>,
        ExecutionState<'static, Arm64<User>>,
        ExecutionState<'static, Arm64<User>>,
    ),
> {
    RECORDING.generate_cases(&GENERATOR)
}

wcontainers_precise_instruction_set_conformance::tests!(
    Measured,
    measure_recording(),
    recorded_cases()
);
