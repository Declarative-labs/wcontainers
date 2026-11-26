//! The inputs of the fixed forms: the form's sample encoding with its memory operand moved to every placement the
//! form's row allows, INT n's vector, every code layout and every flag. The registers the sample names stay, so that
//! every state encodes the form itself; no fixed form's outcome depends on which they are.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Decoder, DecoderOptions, Instruction as Decoded, OpKind};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_x86_64::fixed::{FixedForm, FixedOutcome, MemoryVariation};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Instruction, InstructionFetch, PagePermission, Processor, SAMPLE_ENCODINGS, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{Addressing, CodeLayout, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::{BITNESS, CODE_ADDRESS, FlagSet, ImmediateClass, OperandType, OperandValue, Splits};

/// UD0 as Intel's chips read it, with a ModRM byte, which XED's sample leaves out.
const UNDEFINED_ZERO: [u8; 3] = [0x0F, 0xFF, 0xC0];

/// The form's sample bytes.
fn to_sample(form: &FixedForm) -> &'static [u8] {
    match form.instruction {
        Instruction::Ud0 => &UNDEFINED_ZERO,
        instruction => SAMPLE_ENCODINGS[instruction as usize],
    }
}

/// The form's sample, decoded at the code address.
fn decode_sample(form: &FixedForm) -> Decoded {
    Decoder::with_ip(BITNESS, to_sample(form), CODE_ADDRESS, DecoderOptions::NONE).decode()
}

fn has_memory_operand(decoded: &Decoded) -> bool {
    (0..decoded.op_count()).any(|index| decoded.op_kind(index) == OpKind::Memory)
}

/// The operand types of a fixed form: its memory operand where the row lets it move, else its code layouts; INT n's
/// vector; the flags.
pub fn list_operand_types(form: &FixedForm) -> Vec<OperandType> {
    let decoded = decode_sample(form);
    let mut types = Vec::new();
    if form.memory != MemoryVariation::Sample && has_memory_operand(&decoded) {
        types.push(OperandType::Memory { splits: Splits::Allowed, offset: false });
    } else {
        types.push(OperandType::Encoding(0));
    }
    if form.outcome == FixedOutcome::SoftwareInterrupt && decoded.op_count() > 0 {
        types.push(OperandType::Immediate(ImmediateClass::Vector));
    }
    types.push(OperandType::Flags(FlagSet::Unread));
    types
}

/// Builds the state of a fixed form from one value of each of its operand types.
pub fn build_state(form: &FixedForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut decoded = decode_sample(form);
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let mut layout = CodeLayout::PageStart;
    let mut memory_mutations = Vec::new();
    let mut memory_operand = None;
    let mut changed = false;
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => layout = encoding.layout,
            (OperandType::Memory { .. }, OperandValue::Access(mut access)) => {
                layout = access.layout;
                // Relative to the instruction pointer the 0x0F 0x18 forms would be PREFETCHIT0 and PREFETCHIT1.
                if form.memory == MemoryVariation::NotInstructionPointerRelative && access.addressing == Addressing::InstructionPointer {
                    access.addressing = Addressing::Displacement;
                }
                let width = (decoded.memory_size().size() as u64).max(1);
                let (operand, _) = place_memory_operand(access, width, &mut processor, &mut memory_mutations);
                decoded.set_memory_base(operand.base);
                decoded.set_memory_index(operand.index);
                decoded.set_memory_index_scale(operand.scale);
                decoded.set_memory_displacement64(operand.displacement as u64);
                decoded.set_memory_displ_size(operand.displ_size);
                decoded.set_segment_prefix(operand.segment_prefix);
                memory_operand = Some(operand);
                changed = true;
            }
            (OperandType::Immediate(_), OperandValue::Immediate(vector)) => {
                decoded.set_immediate8(vector as u8);
                changed = true;
            }
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let encoded = if changed {
        encode_at_code_address(&decoded)
    } else {
        to_sample(form).to_vec()
    };
    let (code_page, start, bytes) = lay_out(layout, &encoded, to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}
