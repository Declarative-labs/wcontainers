use alloc::vec::Vec;

use iced_x86::{Decoder, DecoderOptions};

use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::{Flags, SAMPLE_ENCODINGS, X86_64, list_modeled_forms};

use crate::{BITNESS, X86_64InputDomain};

/// The longest instruction; the generator pads some to one byte more.
const MAXIMUM_INSTRUCTION_BYTES: usize = 15;

/// Every state of a modeled form holds one instruction at its instruction pointer, all of it but where it is too long
/// or runs past the program's addresses.
#[test]
fn every_state_holds_one_instruction_at_its_instruction_pointer() {
    for instruction in list_modeled_forms() {
        for state in X86_64InputDomain.generate_critical_states(instruction) {
            let Some(MemoryDelta::ValueAssignment { start, bytes }) = state.memory_mutations.get(1) else {
                panic!("the code comes second");
            };
            assert_eq!(*start, state.processor.instruction_pointer);
            let decoded = Decoder::with_ip(BITNESS, bytes, *start, DecoderOptions::NONE).decode();
            let too_long = bytes.len() > MAXIMUM_INSTRUCTION_BYTES;
            let past_the_edge = *start + bytes.len() as u64 == X86_64::ADDRESSABLE.end && decoded.is_invalid();
            assert!(
                too_long || past_the_edge || (!decoded.is_invalid() && decoded.len() == bytes.len()),
                "{instruction:?}: {bytes:02x?}"
            );
        }
    }
}

/// Every state of a form not yet modeled holds the form's sample at its instruction pointer, which `forms_source`
/// decodes with XED.
#[test]
fn every_other_state_holds_its_sample_at_its_instruction_pointer() {
    let modeled = list_modeled_forms();
    for instruction in X86_64::list_instructions().filter(|instruction| !modeled.contains(instruction)) {
        for state in X86_64InputDomain.generate_critical_states(instruction) {
            let Some(MemoryDelta::ValueAssignment { start, bytes }) = state.memory_mutations.get(1) else {
                panic!("the code comes second");
            };
            assert_eq!(*start, state.processor.instruction_pointer);
            assert_eq!(&bytes[..], SAMPLE_ENCODINGS[instruction as usize], "{instruction:?}");
        }
    }
}

#[test]
fn every_state_holds_only_flags_a_program_can_set_and_only_its_own_addresses() {
    for instruction in X86_64::list_instructions() {
        for state in X86_64InputDomain.generate_critical_states(instruction) {
            assert!(Flags::all().contains(state.processor.flags));
            for delta in &state.memory_mutations {
                let addresses = match delta {
                    MemoryDelta::PermissionAssignment { addresses, .. } => addresses.clone(),
                    MemoryDelta::ValueAssignment { start, bytes } => *start..*start + bytes.len() as u64,
                };
                assert!(
                    X86_64::ADDRESSABLE.contains(&addresses.start) && addresses.end <= X86_64::ADDRESSABLE.end,
                    "{instruction:?}: {addresses:#x?}"
                );
            }
        }
    }
}

/// Each exception pair reaches its exception, exactly or not as it is meant to: with every exception unmasked the
/// pairs of MUL and DIV raise overflow exactly, overflow inexactly, underflow exactly, underflow inexact only once
/// denormalized (so exact to an unmasked underflow) and underflow inexactly, those of ADD and SUB the first three,
/// and their sign-flipped copies the same.
#[test]
fn every_exception_pair_reaches_its_exception() {
    use wcontainers_processor_architecture_x86_64::float::{
        Arrangement, FLOAT_FORMS, FloatOperation, INEXACT, OVERFLOW, UNDERFLOW,
    };

    use crate::float::generate_exception_pairs;
    use crate::vector::{fill_lanes, to_float_lane};

    for form in FLOAT_FORMS.iter().filter(|form| form.arrangement == Arrangement::Packed) {
        let pairs = generate_exception_pairs(form.operation, form.precision);
        let expected: &[u32] = match form.operation {
            FloatOperation::Multiply | FloatOperation::Divide => {
                &[OVERFLOW, OVERFLOW | INEXACT, UNDERFLOW, UNDERFLOW, UNDERFLOW | INEXACT]
            }
            FloatOperation::Add | FloatOperation::Subtract => &[OVERFLOW, OVERFLOW | INEXACT, UNDERFLOW],
            FloatOperation::SquareRoot | FloatOperation::Minimum | FloatOperation::Maximum => &[],
            _ => continue,
        };
        assert_eq!(pairs.len(), 2 * expected.len(), "{:?}", form.instruction);
        let lane = to_float_lane(form.precision);
        for (index, (first, second)) in pairs.into_iter().enumerate() {
            let flags = form
                .apply(fill_lanes(lane, first), fill_lanes(lane, second), 0, 0)
                .expect_err("an unmasked exception raises #XM");
            assert_eq!(flags, expected[index % expected.len()], "{:?} pair {index}", form.instruction);
        }
    }
}

/// Each end of every lane's signed and unsigned range fills the first vector operand of every integer vector form.
#[test]
fn every_lane_range_end_fills_a_vector() {
    use wcontainers_processor_architecture_x86_64::vector::VECTOR_FORMS;

    use crate::vector::fill_lanes;
    use crate::{OperandType, OperandValue};

    for form in VECTOR_FORMS {
        let lane = form.operation.to_lane();
        let bits = lane.to_bits();
        let mask = if bits == u64::BITS { u64::MAX } else { (1 << bits) - 1 };
        let ends = [0, mask >> 1, (mask >> 1) + 1, mask].map(|end| fill_lanes(lane, end));
        let types: Vec<OperandType> = X86_64InputDomain.list_operand_types(form.instruction).collect();
        let vectors = types.iter().find(|kind| matches!(kind, OperandType::VectorValue(..)));
        let values: Vec<u128> = X86_64InputDomain
            .generate_critical_values(*vectors.expect("a vector operand"))
            .map(|value| match value {
                OperandValue::Vector(value) => value,
                other => panic!("{other:?} is no vector"),
            })
            .collect();
        for end in ends {
            assert!(values.contains(&end), "{:?} lacks {end:#x}", form.instruction);
        }
    }
}
