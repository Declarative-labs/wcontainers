use std::path::Path;

use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_simulation::Simulation;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;

/// What an Apple M5 did with each listed instruction, written by the record tool.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/apple-m5"
);

type Generator = CriticalValueProduct<Arm64InputDomain>;
type AppleM5 = MeasuredInstructionSet<Recording, Generator>;

/// Returns the behaviors the Apple M5 showed, measured from its recording.
fn measure_apple_m5() -> AppleM5 {
    let recording = match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    };
    match AppleM5::measure(recording) {
        Ok(set) => set,
        Err(_) => panic!("the recording shows no one known behavior for some instruction"),
    }
}

wcontainers_executor_conformance::tests!(
    Simulation::new(measure_apple_m5()),
    measure_apple_m5(),
    Generator::new(Arm64InputDomain)
);
