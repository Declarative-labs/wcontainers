use std::path::Path;

use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;

/// What a Neoverse V2 under KVM did, written by the record tool.
const RECORDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recordings/neoverse-v2");

fn recording() -> Recording {
    match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    }
}

wcontainers_behavior_source_conformance::tests!(recording(), CriticalValueCut::default());
