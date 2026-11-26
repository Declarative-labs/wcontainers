//! Bakes a sample of the accumulator machine through the code that bakes a collected sample.
//!
//! Its core shows each instruction's last known behavior; the file goes into `samples/accumulator-machine/`, named
//! after the kind of core as the command names a collected sample's file.

use std::fs;
use std::path::Path;

use wcontainers_observation_collector::CoreSample;
use wcontainers_precise_instruction_set_collected::bake_sample;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_accumulator_machine::{
    AccumulatorMachineProcessorArchitecture, CoreDesign, CoreIdentity, CoreRevision,
};

/// The folder of the file, which is named after the kind of core.
const SAMPLE_FOLDER: &str = "samples/accumulator-machine";
const BAKED_EXTENSION: &str = "rs";
/// What the file's top says of where the sample came from, in place of the labels of a machine.
const ORIGIN: &str = "No chip was measured: each instruction shows its last known behavior, as \
                      the example bake_accumulator_machine_sample chooses.";

fn main() {
    let measurements = AccumulatorMachineProcessorArchitecture::list_instructions()
        .filter_map(|instruction| {
            AccumulatorMachineProcessorArchitecture::list_known_behaviors(instruction)
                .last()
                .map(|precise| (instruction, precise))
        })
        .collect();
    let sample: CoreSample<AccumulatorMachineProcessorArchitecture, CoreIdentity> = CoreSample {
        core: CoreIdentity {
            design: CoreDesign::Reference,
            revision: CoreRevision::Second,
        },
        measurements,
    };
    let source = match bake_sample(&sample, &[ORIGIN.to_owned()]) {
        Ok(source) => source,
        Err(error) => panic!("the sample does not bake: {error:?}"),
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(SAMPLE_FOLDER)
        .join(sample.core.to_kind_name())
        .with_extension(BAKED_EXTENSION);
    if let Err(error) = fs::write(&path, source) {
        panic!("{} is not writable: {error}", path.display());
    }
    println!("wrote {}", path.display());
}
