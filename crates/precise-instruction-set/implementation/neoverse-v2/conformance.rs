use std::path::Path;

use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
use wcontainers_processor_architecture_arm64::Arm64;
use wcontainers_processor_architecture_recording::{Recording, read_recordings};

/// What a Neoverse V2 under KVM did, written by the recording crate's record tool.
const RECORDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recordings");

fn recordings() -> Vec<Recording<Arm64>> {
    match read_recordings(Path::new(RECORDINGS)) {
        Ok(recordings) => recordings,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

wcontainers_precise_instruction_set_conformance::tests!(NeoverseV2, recordings());
