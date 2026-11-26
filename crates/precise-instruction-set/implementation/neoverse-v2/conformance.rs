use std::path::Path;

use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction};
use wcontainers_processor_architecture_recording::read_recordings;

/// What a Neoverse V2 under KVM did, written by the measured set's record tool.
const RECORDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recordings");

fn recorded_cases() -> Vec<(
    Instruction,
    ExecutionState<'static, Arm64>,
    ExecutionState<'static, Arm64>,
)> {
    let recordings = match read_recordings(Path::new(RECORDINGS)) {
        Ok(recordings) => recordings,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    };
    recordings
        .iter()
        .flat_map(|recording| {
            let instruction = recording.instruction();
            recording
                .cases()
                .iter()
                .map(move |(before, after)| (instruction, before.clone(), after.clone()))
        })
        .collect()
}

wcontainers_precise_instruction_set_conformance::tests!(NeoverseV2, recorded_cases());
