//! The inputs of the conversions between MMX and XMM registers and of MASKMOVQ: the registers and code layout or the
//! memory operand, the x87 state MMX shares, MXCSR, the source's two lanes, and for MASKMOVQ where DS:RDI points, the
//! data and the mask.
//!
//! The source's lanes are the SSE conversion's own edges and each class of value, each in the low lane with the next
//! one in the high lane, so that both lanes meet every value.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Instruction as Encoded, MemoryOperand, Register};
use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::float::{Conversion, Precision};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::mmx_convert::{MmxConvertForm, MmxConvertOperation};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

use crate::float::{MxcsrSet, generate_conversion_edges};
use crate::integer::{CodeLayout, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::mmx::X87Environment;
use crate::sse::{SseOperand, SseValue};
use crate::{
    AccessPlacement, AddressSize, CODE_ADDRESS, FlagSet, NON_CANONICAL_ADDRESS, OperandType, OperandValue, SegmentChoice,
    VECTOR, ValueSet, X86_64InputDomain, to_access_address, to_data_pages,
};

/// What a form's data operand holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConvertSet {
    /// The two lanes a conversion reads.
    Source { conversion: Conversion, precision: Precision },
    /// MASKMOVQ's mask register.
    Mask,
}

const MMX: [Register; 8] = [Register::MM0, Register::MM1, Register::MM2, Register::MM3, Register::MM4, Register::MM5, Register::MM6, Register::MM7];
/// MASKMOVQ's data: every byte different and none zero.
const MASKED_DATA: u64 = 0x0123_4567_89AB_CDEF;
const DESTINATION_INDEX: usize = 7;
/// An FS base MASKMOVQ's RDI makes up for.
const MASKED_FS_BASE: u64 = 0x0000_7000_0000_0000;
/// What RDI holds above EDI when the 0x67 prefix makes MASKMOVQ read EDI.
const ABOVE_ADDRESS_HALF: u64 = 0xA5A5_A5A5_0000_0000;

/// The operand types of a form of this family, in the order the build reads them.
pub fn list_operand_types(form: &MmxConvertForm) -> Vec<OperandType> {
    let flags = OperandType::Flags(FlagSet::Unread);
    match form.operation {
        MmxConvertOperation::MaskedStore => vec![
            OperandType::Encoding(0),
            OperandType::Sse(SseOperand::MaskedDestination),
            OperandType::X87Environment,
            OperandType::ConvertSource(ConvertSet::Mask),
            flags,
        ],
        MmxConvertOperation::Convert(conversion) => {
            let place = if form.place == wcontainers_processor_architecture_x86_64::integer::OperandPlace::Register {
                OperandType::Encoding(0)
            } else {
                OperandType::Memory { splits: crate::Splits::Allowed, offset: false }
            };
            vec![
                place,
                OperandType::X87Environment,
                OperandType::Mxcsr(MxcsrSet::Arithmetic),
                OperandType::ConvertSource(ConvertSet::Source { conversion, precision: form.precision }),
                flags,
            ]
        }
    }
}

/// A source's lane values: its conversion's edges and every class of value; or MASKMOVQ's masks: none, every byte,
/// every other byte, the lowest and the highest alone, and every bit but the top ones.
pub fn generate_sources(set: ConvertSet) -> Vec<OperandValue> {
    let ConvertSet::Source { conversion, precision } = set else {
        return [0, 0x8080_8080_8080_8080, 0x0080_0080_0080_0080, 0x80, 0x8000_0000_0000_0000, 0x7F7F_7F7F_7F7F_7F7F]
            .map(OperandValue::Vector)
            .to_vec();
    };
    let mut lanes = generate_conversion_edges(conversion, precision);
    let (bits, classes) = match (conversion, precision) {
        (Conversion::IntegerToFloat, _) => {
            (u32::BITS, vec![0, 1, u64::from(u32::MAX), 0x8000_0000, 0x7FFF_FFFF, 0x0123_4567])
        }
        (_, Precision::Single) => (u32::BITS, read_bits(ValueSet::SingleFloat)),
        (_, Precision::Double) => (u64::BITS, read_bits(ValueSet::DoubleFloat)),
        (_, Precision::Half) => unreachable!("no conversion that names an MMX register reads halves"),
    };
    lanes.extend(classes);
    let count = lanes.len();
    (0..count)
        .map(|index| {
            let (low, high) = (u128::from(lanes[index]), u128::from(lanes[(index + 1) % count]));
            OperandValue::Vector(low | high << bits)
        })
        .collect()
}

fn read_bits(set: ValueSet) -> Vec<u64> {
    X86_64InputDomain
        .generate_critical_values(OperandType::MemoryValue(set))
        .filter_map(|value| match value {
            OperandValue::Bits(bits) => Some(bits),
            _ => None,
        })
        .collect()
}

/// Builds the state of a form of this family from one value of each of its operand types.
pub fn build_state(form: &MmxConvertForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut registers, mut access, mut layout, mut source, mut destination) = ([0, 0], None, CodeLayout::PageStart, 0, None);
    let mut environment = X87Environment::Initial;
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => (registers, layout) = (encoding.registers, encoding.layout),
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                registers[0] = chosen.register;
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::Sse(SseOperand::MaskedDestination), OperandValue::Sse(value)) => destination = Some(value),
            (OperandType::X87Environment, OperandValue::X87(chosen)) => environment = chosen,
            (OperandType::Mxcsr(_), OperandValue::Mxcsr(mxcsr)) => processor.mxcsr = mxcsr,
            (OperandType::ConvertSource(_), OperandValue::Vector(value)) => source = value,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    processor.x87 = environment.to_x87();
    let mut memory_mutations = Vec::new();
    let (encoded, padding) = match form.operation {
        MmxConvertOperation::MaskedStore => {
            let (data, mask) = (MMX[registers[0] % MMX.len()], MMX[(registers[1] + 1) % MMX.len()]);
            let exponent = processor.x87.registers[0] & !u128::from(u64::MAX);
            processor.x87.registers[data.number()] = exponent | u128::from(MASKED_DATA);
            processor.x87.registers[mask.number()] = exponent | (source & u128::from(u64::MAX));
            let Some(SseValue::MaskedDestination { placement, lower, upper, address_size, segment }) = destination else {
                panic!("MASKMOVQ has a destination");
            };
            memory_mutations.extend(to_data_pages(lower, upper));
            let address = match placement {
                AccessPlacement::NonCanonical => NON_CANONICAL_ADDRESS,
                placement => to_access_address(placement, 8),
            };
            let (segment_prefix, base) = if segment == SegmentChoice::Fs { (Register::FS, MASKED_FS_BASE) } else { (Register::None, 0) };
            processor.fs_base = base;
            let offset = address.wrapping_sub(base);
            let (bits, rdi) = match address_size {
                AddressSize::Bits64 => (u64::BITS, offset),
                AddressSize::Bits32 => (u32::BITS, ABOVE_ADDRESS_HALF | offset),
            };
            processor.general_purpose_registers[DESTINATION_INDEX] = rdi;
            let encoded = Encoded::with_maskmovq(bits, data, mask, segment_prefix)
                .unwrap_or_else(|error| panic!("MASKMOVQ takes its operands: {error}"));
            let mut padding_operand = MemoryOperand::with_base(Register::RDI);
            padding_operand.segment_prefix = segment_prefix;
            (encode_at_code_address(&encoded), to_padding(Some(padding_operand)))
        }
        MmxConvertOperation::Convert(_) => {
            let to_mmx = form.code.op_code().op_kind(0) == iced_x86::OpCodeOperandKind::mm_reg;
            let xmm = VECTOR[registers[0]];
            let mmx = MMX[registers[1] % MMX.len()];
            let (first, second) = if to_mmx { (mmx, xmm) } else { (xmm, mmx) };
            match access {
                Some(access) => {
                    let width = form.memory_bytes();
                    let (operand, address) = place_memory_operand(access, width, &mut processor, &mut memory_mutations);
                    let bytes: Vec<u8> = (0..width)
                        .take_while(|offset| X86_64::ADDRESSABLE.contains(&address.wrapping_add(*offset)))
                        .map(|offset| source.to_le_bytes()[offset as usize])
                        .collect();
                    if !bytes.is_empty() {
                        memory_mutations.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(bytes) });
                    }
                    let encoded = Encoded::with2(form.code, first, operand)
                        .unwrap_or_else(|error| panic!("{:?} takes its operands: {error}", form.code));
                    (encode_at_code_address(&encoded), to_padding(Some(operand)))
                }
                None => {
                    if to_mmx {
                        processor.vector_registers[xmm.number()] = source;
                    } else {
                        let exponent = processor.x87.registers[mmx.number()] & !u128::from(u64::MAX);
                        processor.x87.registers[mmx.number()] = exponent | (source & u128::from(u64::MAX));
                    }
                    let encoded = Encoded::with2(form.code, first, second)
                        .unwrap_or_else(|error| panic!("{:?} takes its operands: {error}", form.code));
                    (encode_at_code_address(&encoded), to_padding(None))
                }
            }
        }
    };
    let (code_page, start, bytes) = lay_out(layout, &encoded, padding);
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}
