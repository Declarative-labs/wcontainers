extern crate std;

use alloc::vec::Vec;

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::{Arm64, User};

use crate::CollectedPreciseInstructionSet;
use crate::arm64::SAMPLES;

type Architecture = Arm64<User>;

/// Checks each baked ARM64 sample's core against its file name, its order against the listing, and each behavior.
#[test]
fn every_arm64_sample_holds_each_listed_instruction_with_a_known_behavior() {
    assert!(!SAMPLES.is_empty(), "no ARM64 sample is baked");
    for (file_name, sample) in SAMPLES {
        assert_eq!(sample.core.to_kind_name(), *file_name);
        assert!(
            sample
                .measurements
                .iter()
                .map(|(instruction, _)| *instruction)
                .eq(Architecture::list_instructions()),
            "sample {file_name} does not hold every instruction once in listed order"
        );
        for &(instruction, behavior) in sample.measurements {
            assert!(
                Architecture::list_known_behaviors(instruction).any(|known| known == behavior),
                "sample {file_name} gives {instruction:?} a behavior that is not a known one"
            );
        }
    }
}

/// Builds a set from this chip's baked sample and from a fresh measurement with the collector's generator, and
/// checks that they give each listed instruction the same behavior; it runs only on an ARM64 host under KVM.
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
#[test]
#[ignore = "needs KVM on a chip whose kind has a baked sample"]
fn this_chip_measures_as_its_baked_sample() {
    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
    use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
    use wcontainers_processor_cores::ProcessorCores;
    use wcontainers_processor_cores_linux::LinuxProcessorCores;

    let cores = match LinuxProcessorCores.list_cores() {
        Ok(cores) => cores,
        Err(error) => panic!("the cores of this host cannot be listed: {error:?}"),
    };
    let Some((_, identity)) = cores.first() else {
        panic!("this host lists no core");
    };
    let Some((file_name, sample)) = SAMPLES.iter().find(|(_, sample)| sample.core == *identity)
    else {
        panic!("no sample is baked for {}", identity.to_kind_name());
    };
    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let fresh = match chip.measure_instructions(&CriticalValueCut::default()) {
        Ok(fresh) => fresh,
        Err(_) => panic!("the fresh measurement failed; collect-observations writes its evidence"),
    };
    let baked_set =
        CollectedPreciseInstructionSet::<Architecture>::from_measurements(sample.measurements);
    let fresh_set = CollectedPreciseInstructionSet::<Architecture>::from_measurements(&fresh);
    let differing: Vec<_> = Architecture::list_instructions()
        .filter(|&instruction| {
            baked_set.to_precise(instruction) != fresh_set.to_precise(instruction)
        })
        .collect();
    assert!(
        differing.is_empty(),
        "{} instructions measure otherwise than sample {file_name} says, the first {:?}",
        differing.len(),
        differing.first()
    );
}
