extern crate std;

use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_accumulator_machine::AccumulatorMachineInstructionInputDomain;
use wcontainers_processor_architecture::{
    MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_accumulator_machine::Processor;

use crate::BakedSample;

/// A source that shows one behavior: whatever state it executes, it leaves what that behavior expects.
struct SimulatedBehavior<Architecture: ProcessorArchitecture> {
    behavior: Architecture::PreciseInstruction,
    /// Returns a state that meets the expectation, the one a chip of the behavior could leave.
    to_state: fn(&Architecture::ProcessorStateExpectation) -> Architecture::ProcessorState,
}

impl<Architecture> BehaviorSource for SimulatedBehavior<Architecture>
where
    Architecture: ProcessorArchitecture,
    Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
{
    type Architecture = Architecture;
    type Error = Infallible;

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Architecture::ProcessorState,
        memory: MemorySnapshot<'_, '_, Architecture>,
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> Result<(), Infallible> {
        let Ok(expectation) = self.behavior.simulate(processor, memory, mutation_hook);
        *processor = (self.to_state)(&expectation);
        Ok(())
    }
}

/// Checks each sample's order and behaviors, and that a source showing them measures back to them.
///
/// Each instruction appears once in listed order, each behavior is a known one, and the generator is the one the
/// collector uses.
fn check_samples<Architecture, CoreIdentity>(
    samples: &[BakedSample<Architecture, CoreIdentity>],
    generator: &impl DistinguishingStateGenerator<Architecture>,
    to_state: fn(&Architecture::ProcessorStateExpectation) -> Architecture::ProcessorState,
) where
    Architecture: ProcessorArchitecture,
    Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
{
    for (sample_index, sample) in samples.iter().enumerate() {
        let sampled: Vec<Architecture::Instruction> = sample
            .measurements
            .iter()
            .map(|(instruction, _)| *instruction)
            .collect();
        assert!(
            sampled
                .iter()
                .copied()
                .eq(Architecture::list_instructions()),
            "sample {sample_index} does not hold every instruction once in listed order"
        );
        for (index, &(instruction, behavior)) in sample.measurements.iter().enumerate() {
            assert!(
                Architecture::list_known_behaviors(instruction).any(|known| known == behavior),
                "instruction {index} of sample {sample_index} has a behavior that is not a known one"
            );
            let mut source = SimulatedBehavior { behavior, to_state };
            match source.measure_instruction(instruction, generator) {
                Ok(measured) => assert!(
                    measured == behavior,
                    "instruction {index} of sample {sample_index} measures as another behavior"
                ),
                Err(MeasureInstructionError::IndistinguishableBehaviors { behaviors, .. }) => {
                    panic!(
                        "no generated state tells the behavior of instruction {index} of sample {sample_index} from {} others",
                        behaviors.len() - 1
                    )
                }
                Err(_) => {
                    panic!("instruction {index} of sample {sample_index} does not measure back")
                }
            }
        }
    }
}

#[test]
fn every_baked_sample_measures_back() {
    check_samples(
        crate::accumulator_machine::SAMPLES,
        &CriticalValueProduct::new(AccumulatorMachineInstructionInputDomain),
        Processor::clone,
    );
}
