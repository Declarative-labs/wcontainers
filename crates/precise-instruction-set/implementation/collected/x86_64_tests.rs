extern crate std;

use alloc::vec::Vec;

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_x86_64::X86_64;

use crate::CollectedPreciseInstructionSet;
use crate::x86_64::SAMPLES;

/// Checks each baked x86-64 sample's core against its file name, its order against the listing, and each behavior.
#[test]
fn every_x86_64_sample_holds_each_listed_instruction_with_a_known_behavior() {
    assert!(!SAMPLES.is_empty(), "no x86-64 sample is baked");
    for (file_name, sample) in SAMPLES {
        assert_eq!(sample.core.to_kind_name(), *file_name);
        assert!(
            sample
                .measurements
                .iter()
                .map(|(instruction, _)| *instruction)
                .eq(X86_64::list_instructions()),
            "sample {file_name} does not hold every instruction once in listed order"
        );
        for &(instruction, behavior) in sample.measurements {
            assert!(
                X86_64::list_known_behaviors(instruction).any(|known| known == behavior),
                "sample {file_name} gives {instruction:?} a behavior that is not a known one"
            );
        }
    }
}

/// Builds a set from this chip's baked sample and from a fresh measurement on one core with the chip source's reference
/// choices, and checks that they give each listed instruction the same behavior; it runs only on an x86-64 host under
/// KVM.
#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
#[test]
#[ignore = "needs KVM on a chip whose kind has a baked sample"]
fn this_chip_measures_as_its_baked_sample() {
    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_behavior_source_kvm_single_step_x86_64::{Options, SingleStepMachine};
    use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
    use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
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
    let mut chip = match SingleStepMachine::with_options(Options::PLAIN) {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let fresh = match chip.measure_instructions(&CriticalValueCut::new(X86_64InputDomain)) {
        Ok(fresh) => fresh,
        Err(_) => panic!("the fresh measurement failed; collect-observations writes its evidence"),
    };
    let baked_set = CollectedPreciseInstructionSet::<X86_64>::from_measurements(sample.measurements);
    let fresh_set = CollectedPreciseInstructionSet::<X86_64>::from_measurements(&fresh);
    let differing: Vec<_> = X86_64::list_instructions()
        .filter(|&instruction| baked_set.to_precise(instruction) != fresh_set.to_precise(instruction))
        .collect();
    assert!(
        differing.is_empty(),
        "{} instructions measure otherwise than sample {file_name} says, the first {:?}",
        differing.len(),
        differing.first()
    );
}
