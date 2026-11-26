//! The tests every implementation of wcontainers-observation-collector runs, written against its traits alone.

use wcontainers_observation_collector::ObservationCollector;
use wcontainers_processor_architecture::ProcessorArchitecture;

/// Checks that the machine gives at least one sample and that no kind of core is sampled twice.
pub fn collects_one_sample_for_each_kind_of_core<Collector: ObservationCollector>(
    collector: Collector,
) {
    let Ok(samples) = collector.collect_samples() else {
        panic!("the samples could not be collected");
    };
    assert!(!samples.is_empty());
    for (position, sample) in samples.iter().enumerate() {
        assert!(
            !samples[..position]
                .iter()
                .any(|other| other.core == sample.core)
        );
    }
}

/// Checks that each sample holds every instruction of the architecture once, in the order the architecture lists them.
pub fn samples_hold_every_instruction_once<Collector: ObservationCollector>(collector: Collector) {
    let Ok(samples) = collector.collect_samples() else {
        panic!("the samples could not be collected");
    };
    for sample in &samples {
        let mut measured = sample
            .measurements
            .iter()
            .map(|(instruction, _)| *instruction);
        for listed in <Collector::Architecture as ProcessorArchitecture>::list_instructions() {
            assert!(measured.next() == Some(listed));
        }
        assert!(measured.next().is_none());
    }
}

/// Runs each suite function above as its own test on a value made anew for every test.
///
/// List every suite function here, in the order above, and nothing else.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn collects_one_sample_for_each_kind_of_core() {
            $crate::collects_one_sample_for_each_kind_of_core($value);
        }

        #[test]
        fn samples_hold_every_instruction_once() {
            $crate::samples_hold_every_instruction_once($value);
        }
    };
}
