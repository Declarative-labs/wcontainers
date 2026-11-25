//! The tests every implementation of wcontainers-precise-instruction-set runs, written against its traits alone.

use std::convert::Infallible;

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};

/// The set built from measurements gives each measured instruction the behavior measured for it.
pub fn gives_each_instruction_its_measured_behavior<Architecture, Set>(
    measurements: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
) where
    Architecture: ProcessorArchitecture,
    Set: PreciseInstructionSet<Architecture>,
{
    let set = Set::from_measurements(&measurements);
    for (index, (instruction, precise)) in measurements.into_iter().enumerate() {
        assert!(
            set.to_precise(instruction) == precise,
            "measurement {index} is not the behavior the set gives its instruction"
        );
    }
}

/// Measurements given in any order build the same set.
pub fn builds_one_set_from_measurements_in_any_order<Architecture, Set>(
    measurements: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
) where
    Architecture: ProcessorArchitecture,
    Set: PreciseInstructionSet<Architecture>,
{
    let set = Set::from_measurements(&measurements);
    let reversed: Vec<_> = measurements.iter().rev().copied().collect();
    let reversed_set = Set::from_measurements(&reversed);
    for (index, (instruction, _)) in measurements.into_iter().enumerate() {
        assert!(
            set.to_precise(instruction) == reversed_set.to_precise(instruction),
            "the instruction of measurement {index} has another behavior when the measurements are reversed"
        );
    }
}

/// The set built from measurements turns every recorded state into the state the chip gave after.
///
/// The recordings are triples of an instruction, a state before and the chip's state after; they must cover every
/// listed instruction, so that no behavior goes unchecked.
pub fn agrees_with_recordings<Architecture, Set>(
    measurements: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
    recordings: impl IntoIterator<
        Item = (
            Architecture::Instruction,
            ExecutionState<'static, Architecture>,
            ExecutionState<'static, Architecture>,
        ),
    >,
) where
    Architecture: ProcessorArchitecture + Clone,
    Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
    Set: PreciseInstructionSet<Architecture>,
{
    let set = Set::from_measurements(&measurements);
    let mut recorded = Vec::new();
    for (case, (instruction, before, after)) in recordings.into_iter().enumerate() {
        if !recorded.contains(&instruction) {
            recorded.push(instruction);
        }
        let Ok(simulated) = set.to_precise(instruction).simulate(before);
        assert!(
            simulated == after,
            "recorded case {case} disagrees with the set"
        );
    }
    for (index, instruction) in Architecture::list_instructions().enumerate() {
        assert!(
            recorded.contains(&instruction),
            "listed instruction {index} has no recording"
        );
    }
}

/// Runs each suite function above as its own test on measurements and recordings made anew for every test.
///
/// The set is named because measurements are how a set is made, so only the measurements and recordings are values.
#[macro_export]
macro_rules! tests {
    ($set:ty, $measurements:expr, $recordings:expr) => {
        #[test]
        fn gives_each_instruction_its_measured_behavior() {
            $crate::gives_each_instruction_its_measured_behavior::<_, $set>($measurements);
        }

        #[test]
        fn builds_one_set_from_measurements_in_any_order() {
            $crate::builds_one_set_from_measurements_in_any_order::<_, $set>($measurements);
        }

        #[test]
        fn agrees_with_recordings() {
            $crate::agrees_with_recordings::<_, $set>($measurements, $recordings);
        }
    };
}
