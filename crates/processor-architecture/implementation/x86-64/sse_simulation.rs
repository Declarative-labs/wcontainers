//! The formulas of the forms of [`crate::sse`], on the address, access and register helpers of [`super`].
//!
//! A memory operand that must be aligned to 16 bytes and is not raises #GP(0) before any other check, as for the
//! lane skeleton; every other access is checked as [`super::check_access`] checks it, and a store as
//! [`super::store`] writes it. A floating-point form that finds an unmasked exception adds the flags it found to
//! MXCSR, raises #XM and writes nothing else.

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, SSE_ALIGNMENT, check_access, expect_code, locate, read_register, read_value, store, write_register};
use crate::behavior::StoreAcrossFault;
use crate::float::sse_float::{ConvertedLane, compute_dot_product, convert_integer, convert_to_integer, resolve_flags, round_integral};
use crate::float::{Arrangement, Precision};
use crate::memory::read_bytes;
use crate::planted::{Bug, is_active};
use crate::sse::{
    LaneSelect, SseForm, SseOperation, StringControl, StringLength, StringOutput, VECTOR_BYTES,
    accumulate_crc32, compare_strings, find_implicit_length, find_sse_form, to_explicit_length,
};
use crate::vector::{Lane, join, split};
use crate::{Access, Exception, Flags, Instruction, Processor, X86_64};

/// The bits of MXCSR that LDMXCSR may set: those MXCSR_MASK gives a chip with DAZ, the low 16.
const MXCSR_WRITABLE: u32 = u16::MAX as u32;
/// RAX and RDX, which give an explicit-length string compare its lengths.
const FIRST_LENGTH: usize = 0;
const SECOND_LENGTH: usize = 2;

/// Applies a form of this family; a store that faults part-way writes what `across_fault` says below the fault.
pub(super) fn execute_sse(
    instruction: Instruction,
    across_fault: StoreAcrossFault,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_sse_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no form of the SSE family"));
    expect_code(decoded, form.codes);
    let has_immediate = (0..decoded.op_count()).any(|operand| decoded.op_kind(operand) == OpKind::Immediate8);
    let immediate = if has_immediate { decoded.immediate8() } else { 0 };
    match form.operation {
        SseOperation::Extract { bytes, select, .. } => {
            let select = match select {
                LaneSelect::Fixed(_) if is_active(Bug::HighQuadwordStoreReadsLow) => LaneSelect::Fixed(0),
                select => select,
            };
            let lanes = read_xmm(processor, decoded.op1_register()) >> (u8::BITS * select.to_offset(bytes, immediate));
            let value = lanes & to_mask(bytes);
            if decoded.op0_kind() == OpKind::Register {
                let register = decoded.op0_register();
                if is_active(Bug::ExtractKeepsUpperHalf) && register.size() == size_of::<u32>() {
                    let full = &mut processor.general_purpose_registers[register.full_register().number()];
                    *full = (*full & !u64::from(u32::MAX)) | value as u64;
                } else {
                    write_register(processor, register, value as u64);
                }
                return Ok(());
            }
            let operand = locate(processor, decoded);
            let unaligned = is_active(Bug::AlignedStoreMisalignedNoFault) && instruction == Instruction::MovapsMempsXmmps;
            if !unaligned {
                check_alignment(form, operand)?;
            }
            store(processor.flags, memory, operand, &value.to_le_bytes()[..bytes as usize], across_fault, on_memory_mutated)
        }
        SseOperation::Insert { bytes, select, zeroing, .. } => {
            let value = match decoded.op1_kind() {
                OpKind::Register => u128::from(read_register(processor, decoded.op1_register())) & to_mask(bytes),
                _ => read_memory(form, processor, memory, decoded, u64::from(bytes))?,
            };
            let number = decoded.op0_register().number();
            let shift = u8::BITS * select.to_offset(bytes, immediate);
            let zeroing = zeroing && !is_active(Bug::InsertZeroingKeepsUpperLanes);
            let kept = if zeroing { 0 } else { processor.vector_registers[number] & !(to_mask(bytes) << shift) };
            processor.vector_registers[number] = kept | (value << shift);
            Ok(())
        }
        SseOperation::MoveMask(lane) => {
            let bit = if is_active(Bug::MoveMaskFromLowBits) { 0 } else { lane.to_bits() - 1 };
            let lanes = split(lane, read_xmm(processor, decoded.op1_register()));
            let mask = lanes.iter().enumerate().fold(0, |mask, (index, value)| mask | ((value >> bit & 1) << index));
            write_register(processor, decoded.op0_register(), mask);
            Ok(())
        }
        SseOperation::StoreGeneral => {
            let register = decoded.op1_register();
            let value = read_register(processor, register);
            store(processor.flags, memory, locate(processor, decoded), &value.to_le_bytes()[..register.size()], across_fault, on_memory_mutated)
        }
        SseOperation::LoadMxcsr => {
            let value = read_memory(form, processor, memory, decoded, form.memory_bytes())? as u32;
            if value & !MXCSR_WRITABLE != 0 && !is_active(Bug::LoadMxcsrReservedNoFault) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            processor.mxcsr = value & MXCSR_WRITABLE;
            Ok(())
        }
        SseOperation::StoreMxcsr => {
            let bytes = processor.mxcsr.to_le_bytes();
            store(processor.flags, memory, locate(processor, decoded), &bytes, across_fault, on_memory_mutated)
        }
        SseOperation::Fence => Ok(()),
        SseOperation::Prefetch if is_active(Bug::PrefetchFaults) => {
            check_access(processor.flags, memory, locate(processor, decoded), form.memory_bytes(), Access::Read)
        }
        SseOperation::Prefetch => Ok(()),
        SseOperation::Combine(combination) => {
            let number = decoded.op0_register().number();
            let source = read_vector_source(form, processor, memory, decoded)?;
            let result = combination.apply(processor.vector_registers[number], source, processor.vector_registers[0], immediate);
            processor.vector_registers[number] = result;
            Ok(())
        }
        SseOperation::CompareStrings { length, output } => {
            compare(form, length, output, processor, memory, decoded, StringControl(immediate))
        }
        SseOperation::Crc32 => {
            let (source, bytes) = match decoded.op1_kind() {
                OpKind::Register => (read_register(processor, decoded.op1_register()), decoded.op1_register().size()),
                _ => {
                    let bytes = decoded.memory_size().size();
                    (read_memory(form, processor, memory, decoded, bytes as u64)? as u64, bytes)
                }
            };
            let register = decoded.op0_register();
            let full = &mut processor.general_purpose_registers[register.full_register().number()];
            let remainder = accumulate_crc32(*full as u32, source, bytes as u32);
            let kept = if is_active(Bug::Crc32KeepsUpperHalf) && register.size() == size_of::<u64>() { *full & !u64::from(u32::MAX) } else { 0 };
            *full = kept | u64::from(remainder);
            Ok(())
        }
        SseOperation::IntegerToFloat(precision) => {
            let integer = match decoded.op1_kind() {
                OpKind::Register => {
                    let register = decoded.op1_register();
                    sign_extend(read_register(processor, register), register.size() as u32)
                }
                _ => {
                    let bytes = form.memory_bytes();
                    sign_extend(read_memory(form, processor, memory, decoded, bytes)? as u64, bytes as u32)
                }
            };
            let lane = convert_integer(integer, precision, processor.mxcsr);
            write_float_lanes(processor, decoded.op0_register().number(), precision, Arrangement::Scalar, &[lane])
        }
        SseOperation::FloatToInteger { precision, truncate } => {
            let source = read_float_source(form, processor, memory, decoded, precision, Arrangement::Scalar)?;
            let register = decoded.op0_register();
            let integer_bits = register.size() as u32 * u8::BITS;
            let lane = convert_to_integer(source as u64, precision, integer_bits, truncate, processor.mxcsr);
            match resolve_flags(&[lane], processor.mxcsr) {
                Ok(flags) => {
                    write_register(processor, register, lane.bits);
                    processor.mxcsr |= flags;
                    Ok(())
                }
                Err(flags) => {
                    processor.mxcsr |= flags;
                    Err(Exception::SimdFloatingPoint)
                }
            }
        }
        SseOperation::Round { precision, arrangement } => {
            let source = read_float_source(form, processor, memory, decoded, precision, arrangement)?;
            let lanes = match arrangement {
                Arrangement::Packed => u128::BITS / precision.to_bits(),
                Arrangement::Scalar => 1,
            };
            let rounded: alloc::vec::Vec<ConvertedLane> = (0..lanes)
                .map(|lane| {
                    let bits = (source >> (lane * precision.to_bits())) as u64 & to_mask(precision.to_bits() / u8::BITS) as u64;
                    round_integral(bits, precision, immediate, processor.mxcsr)
                })
                .collect();
            write_float_lanes(processor, decoded.op0_register().number(), precision, arrangement, &rounded)
        }
        SseOperation::DotProduct(precision) => {
            let source = read_vector_source(form, processor, memory, decoded)?;
            let number = decoded.op0_register().number();
            match compute_dot_product(processor.vector_registers[number], source, precision, immediate, processor.mxcsr) {
                Ok((result, flags)) => {
                    processor.vector_registers[number] = result;
                    processor.mxcsr |= flags;
                    Ok(())
                }
                Err(flags) => {
                    processor.mxcsr |= flags;
                    Err(Exception::SimdFloatingPoint)
                }
            }
        }
    }
}

/// The low `bytes` of 128 bits.
fn to_mask(bytes: u32) -> u128 {
    if bytes >= VECTOR_BYTES { u128::MAX } else { (1 << (bytes * u8::BITS)) - 1 }
}

fn sign_extend(value: u64, bytes: u32) -> i64 {
    let unused = u64::BITS - bytes * u8::BITS;
    ((value << unused) as i64) >> unused
}

fn read_xmm(processor: &Processor, register: Register) -> u128 {
    processor.vector_registers[register.number()]
}

/// #GP(0) where the form requires its memory operand aligned to 16 and it is not.
fn check_alignment(form: &SseForm, operand: MemoryOperand) -> Result<(), Exception> {
    let unaligned = is_active(Bug::NonTemporalLoadUnaligned) && form.instruction == Instruction::MovntdqaXmmdqMemdq;
    if form.requires_alignment() && !operand.address.is_multiple_of(SSE_ALIGNMENT) && !unaligned {
        return Err(Exception::GeneralProtection { error_code: 0 });
    }
    Ok(())
}

/// Reads `bytes` of the memory operand, little-endian.
fn read_memory(form: &SseForm, processor: &Processor, memory: Memory<'_, '_>, decoded: &Decoded, bytes: u64) -> Result<u128, Exception> {
    let operand = locate(processor, decoded);
    check_alignment(form, operand)?;
    check_access(processor.flags, memory, operand, bytes, Access::Read)?;
    if bytes <= size_of::<u64>() as u64 {
        return Ok(u128::from(read_value(memory, operand.address, bytes as usize)));
    }
    Ok(u128::from_le_bytes(read_bytes(memory, operand.address)))
}

/// The second operand of a form of two XMM operands: a register, or 16 bytes of memory.
fn read_vector_source(form: &SseForm, processor: &Processor, memory: Memory<'_, '_>, decoded: &Decoded) -> Result<u128, Exception> {
    match decoded.op1_kind() {
        OpKind::Register => Ok(read_xmm(processor, decoded.op1_register())),
        _ => read_memory(form, processor, memory, decoded, u64::from(VECTOR_BYTES)),
    }
}

/// The source of a floating-point form: a register, or memory as wide as its lanes.
fn read_float_source(
    form: &SseForm,
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    precision: Precision,
    arrangement: Arrangement,
) -> Result<u128, Exception> {
    match decoded.op1_kind() {
        OpKind::Register => Ok(read_xmm(processor, decoded.op1_register())),
        _ => {
            let bytes = match arrangement {
                Arrangement::Packed => u64::from(VECTOR_BYTES),
                Arrangement::Scalar => u64::from(precision.to_bits() / u8::BITS),
            };
            read_memory(form, processor, memory, decoded, bytes)
        }
    }
}

/// Writes the lanes into the register and their flags into MXCSR, a scalar form keeping the lanes above its first;
/// or, where an unmasked exception is found, its flags alone and #XM.
fn write_float_lanes(
    processor: &mut Processor,
    number: usize,
    precision: Precision,
    arrangement: Arrangement,
    lanes: &[ConvertedLane],
) -> Result<(), Exception> {
    match resolve_flags(lanes, processor.mxcsr) {
        Ok(flags) => {
            let bits = precision.to_bits();
            let written = to_mask(lanes.len() as u32 * bits / u8::BITS);
            let zeroes_upper = is_active(Bug::ScalarConversionZeroesUpperLanes) && arrangement == Arrangement::Scalar;
            let kept = if zeroes_upper { 0 } else { processor.vector_registers[number] & !written };
            let value = lanes.iter().enumerate().fold(kept, |all, (lane, outcome)| all | (u128::from(outcome.bits) << (lane as u32 * bits)));
            processor.vector_registers[number] = value;
            processor.mxcsr |= flags;
            Ok(())
        }
        Err(flags) => {
            processor.mxcsr |= flags;
            Err(Exception::SimdFloatingPoint)
        }
    }
}

/// PCMPESTRI to PCMPISTRM: the strings' lengths, the comparison, its flags and its index or mask.
fn compare(
    form: &SseForm,
    length: StringLength,
    output: StringOutput,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    control: StringControl,
) -> Result<(), Exception> {
    let first = read_xmm(processor, decoded.op0_register());
    let second = read_vector_source(form, processor, memory, decoded)?;
    let (first_length, second_length) = match length {
        StringLength::Implicit => (find_implicit_length(first, control), find_implicit_length(second, control)),
        StringLength::Explicit { wide } => {
            let registers = &processor.general_purpose_registers;
            (to_explicit_length(registers[FIRST_LENGTH], wide, control), to_explicit_length(registers[SECOND_LENGTH], wide, control))
        }
    };
    let elements = control.to_elements();
    let result = compare_strings(first, second, first_length, second_length, control);
    let zero_length = if is_active(Bug::StringZeroFlagFromFirst) { first_length } else { second_length };
    let flags = [
        (Flags::CARRY, result != 0),
        (Flags::ZERO, zero_length < elements),
        (Flags::SIGN, first_length < elements),
        (Flags::OVERFLOW, result & 1 != 0),
    ]
    .into_iter()
    .filter(|(_, set)| *set)
    .fold(Flags::empty(), |all, (flag, _)| all | flag);
    processor.flags = processor.flags.difference(Flags::STATUS) | flags;
    match output {
        StringOutput::Index => {
            let most = control.selects_most_or_expands() && !is_active(Bug::StringIndexMostSignificantIgnored);
            let index = match (result, most) {
                (0, _) => elements as u32,
                (_, true) => u32::BITS - 1 - result.leading_zeros(),
                (_, false) => result.trailing_zeros(),
            };
            write_register(processor, Register::ECX, u64::from(index));
        }
        StringOutput::Mask => {
            let lane = if control.is_words() { Lane::Word } else { Lane::Byte };
            let mask = if control.selects_most_or_expands() {
                join(lane, (0..elements).map(|index| if result >> index & 1 != 0 { u64::MAX } else { 0 }))
            } else {
                u128::from(result)
            };
            processor.vector_registers[0] = mask;
        }
    }
    Ok(())
}
