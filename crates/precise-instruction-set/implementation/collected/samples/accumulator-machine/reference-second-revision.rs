// No chip was measured: each instruction shows its last known behavior, as the example bake_accumulator_machine_sample chooses.

pub const SAMPLE: super::Sample = super::Sample {
    core: wcontainers_processor_architecture_accumulator_machine::CoreIdentity {
        design: wcontainers_processor_architecture_accumulator_machine::CoreDesign::Reference,
        revision: wcontainers_processor_architecture_accumulator_machine::CoreRevision::Second,
    },
    measurements: &[
        (
            wcontainers_processor_architecture_accumulator_machine::Instruction::NoOperation,
            wcontainers_processor_architecture_accumulator_machine::PreciseInstruction::NoOperation,
        ),
        (
            wcontainers_processor_architecture_accumulator_machine::Instruction::ClearAccumulator,
            wcontainers_processor_architecture_accumulator_machine::PreciseInstruction::ClearAccumulator,
        ),
        (
            wcontainers_processor_architecture_accumulator_machine::Instruction::Increment,
            wcontainers_processor_architecture_accumulator_machine::PreciseInstruction::Increment(
                wcontainers_processor_architecture_accumulator_machine::IncrementOverflow::Saturates,
            ),
        ),
        (
            wcontainers_processor_architecture_accumulator_machine::Instruction::ShiftLeft,
            wcontainers_processor_architecture_accumulator_machine::PreciseInstruction::ShiftLeft {
                carry: wcontainers_processor_architecture_accumulator_machine::CarryAfterShift::Unchanged,
                low_bit: wcontainers_processor_architecture_accumulator_machine::LowBitAfterShift::Carry,
            },
        ),
    ],
};
