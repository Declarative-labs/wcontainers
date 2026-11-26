//! The skeleton of [`crate::avx`]'s forms: each register source read whole, a memory source read as wide as the
//! form's row says (aligned where it must be, its pages checked as a read), the operation computed, and the first
//! operand written: an XMM register with bits 128 to 255 zeroed, a YMM register whole, a general register as
//! x86-64 writes it, or memory as a store of the row's bytes.

use alloc::vec::Vec;

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, check_access, check_canonical, expect_code, find_fault, locate, read_register, store, write_register};
use crate::avx::{AvxBehavior, AvxForm, AvxOperation, FusedSign, Wide, compare_strings, compute_integer};
use crate::vector::{Lane, join};
use crate::behavior::StoreAcrossFault;
use crate::float::{
    Arrangement, Conversion, FloatForm, FloatOperation, FusedLanes, ORDER_FLAGS, Precision, compare_half, convert_integer, dot_half,
    convert_to_integer, fuse_half, join_halves, narrow_to_halves, round_half, widen_halves,
};
use crate::memory::read_bytes;
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Flags, Processor, VECTOR_REGISTERS, X86_64};

/// The bits of MXCSR this chip implements, which MXCSR_MASK gives: every one of the low 16.
const MXCSR_IMPLEMENTED: u32 = 0xFFFF;

/// A source's bits and how many bytes of it the instruction names.
#[derive(Clone, Copy)]
struct Source {
    bits: Wide,
    bytes: u64,
}

/// Executes the form: reads its sources, computes, and writes its first operand.
pub(super) fn execute_avx(
    form: &AvxForm,
    behavior: AvxBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    expect_code(decoded, &[form.code]);
    match form.operation {
        AvxOperation::ZeroUpper if !is_active(Bug::ZeroUpperKeepsHighRegisters) => {
            processor.upper_vector_halves = [0; VECTOR_REGISTERS];
            return Ok(());
        }
        AvxOperation::ZeroUpper => {
            processor.upper_vector_halves[..VECTOR_REGISTERS / 2].fill(0);
            return Ok(());
        }
        AvxOperation::ZeroAll => {
            processor.vector_registers = [0; VECTOR_REGISTERS];
            processor.upper_vector_halves = [0; VECTOR_REGISTERS];
            return Ok(());
        }
        AvxOperation::MaskedLoad(_) | AvxOperation::MaskedStore(_) | AvxOperation::MaskedByteStore => {
            return execute_masked(form, behavior, processor, memory, decoded, on_memory_mutated);
        }
        _ => {}
    }
    let operands: Vec<u32> = (0..decoded.op_count()).filter(|&index| decoded.op_kind(index) != OpKind::Immediate8).collect();
    let immediate = if operands.len() < decoded.op_count() as usize { decoded.immediate8() } else { 0 };
    let first_source = if form.operation.reads_first_operand() { 0 } else { 1 };
    let mut sources = Vec::new();
    for &index in &operands[first_source..] {
        sources.push(read_source(form, processor, memory, decoded, index)?);
    }
    if let AvxOperation::StringCompare { explicit, index, quadword_lengths } = form.operation {
        let registers = &processor.general_purpose_registers;
        let to_length = |value: u64| if quadword_lengths { value as i64 } else { i64::from(value as u32 as i32) };
        // RAX and RDX are registers 0 and 2.
        let lengths = explicit.then(|| (to_length(registers[0]), to_length(registers[2])));
        let found = compare_strings(sources[0].bits[0], sources[1].bits[0], lengths, immediate);
        let status = [
            (found.result != 0, Flags::CARRY),
            (found.second_short, Flags::ZERO),
            (found.first_short, Flags::SIGN),
            (found.result & 1 != 0, Flags::OVERFLOW),
        ];
        processor.flags = status.iter().filter(|(set, _)| *set).fold(processor.flags.difference(Flags::STATUS), |flags, (_, flag)| flags | *flag);
        if index {
            let position = match found.result {
                0 => found.elements,
                result if immediate & 0x40 != 0 => u16::BITS - 1 - result.leading_zeros(),
                result => result.trailing_zeros(),
            };
            // ECX, whose write clears RCX's upper half.
            processor.general_purpose_registers[1] = u64::from(position);
        } else {
            let lane = if found.elements == 8 { Lane::Word } else { Lane::Byte };
            let mask = if immediate & 0x40 != 0 {
                join(lane, (0..found.elements).map(|element| if found.result >> element & 1 != 0 { u64::MAX } else { 0 }))
            } else {
                u128::from(found.result)
            };
            processor.vector_registers[0] = mask;
            processor.upper_vector_halves[0] = 0;
        }
        return Ok(());
    }
    if form.operation == AvxOperation::LoadMxcsr {
        let value = sources[0].bits[0] as u32;
        if value & !MXCSR_IMPLEMENTED != 0 {
            return Err(Exception::GeneralProtection { error_code: 0 });
        }
        processor.mxcsr = value;
        return Ok(());
    }
    let destination_bits = match decoded.op0_kind() {
        OpKind::Register => decoded.op0_register().size() as u32 * u8::BITS,
        _ => u32::from(form.memory_bytes) * u8::BITS,
    };
    let bits: Vec<Wide> = sources.iter().map(|source| source.bits).collect();
    let computed = match compute_integer(form, &bits, immediate) {
        Some(result) => Ok((result, 0)),
        None => compute_float(form, behavior, &sources, immediate, processor.mxcsr, destination_bits),
    };
    let (value, flags) = match computed {
        Ok(computed) => computed,
        Err(flags) => {
            // A store's page faults come before #XM.
            if decoded.op0_kind() == OpKind::Memory {
                let operand = locate(processor, decoded);
                let bytes = [0; 2 * size_of::<u128>()];
                let flags = to_checked_flags(form, processor.flags);
                store(flags, memory, operand, &bytes[..usize::from(form.memory_bytes)], StoreAcrossFault::WritesNothing, |_| {})?;
            }
            processor.mxcsr |= flags;
            return Err(Exception::SimdFloatingPoint);
        }
    };
    processor.mxcsr |= flags;
    if form.operation.writes_flags() {
        let written = if is_active(Bug::VexTestFlagsKeepStatus) { Flags::empty() } else { Flags::STATUS };
        processor.flags = processor.flags.difference(written) | Flags::from_bits_truncate(value[0] as u64 & ORDER_FLAGS);
        return Ok(());
    }
    match decoded.op0_kind() {
        OpKind::Register => {
            let register = decoded.op0_register();
            if register.is_xmm() || register.is_ymm() {
                let number = register.number();
                processor.vector_registers[number] = value[0];
                processor.upper_vector_halves[number] = if register.is_ymm() { value[1] } else { 0 };
            } else {
                write_register(processor, register, value[0] as u64);
            }
            Ok(())
        }
        _ => {
            let operand = locate(processor, decoded);
            if form.aligned && !operand.address.is_multiple_of(u64::from(form.memory_bytes)) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            let bytes = [value[0].to_le_bytes(), value[1].to_le_bytes()].concat();
            let flags = to_checked_flags(form, processor.flags);
            store(flags, memory, operand, &bytes[..usize::from(form.memory_bytes)], behavior.across_fault, on_memory_mutated)
        }
    }
}

/// Reads one source: a vector register whole, a general register as wide as it is named, or the row's bytes of
/// memory, which must be aligned to their count where the row says so.
fn read_source(form: &AvxForm, processor: &Processor, memory: Memory<'_, '_>, decoded: &Decoded, index: u32) -> Result<Source, Exception> {
    match decoded.op_kind(index) {
        OpKind::Register => {
            let register = decoded.op_register(index);
            if register.is_xmm() || register.is_ymm() {
                let number = register.number();
                return Ok(Source {
                    bits: [processor.vector_registers[number], processor.upper_vector_halves[number]],
                    bytes: register.size() as u64,
                });
            }
            Ok(Source { bits: [u128::from(read_register(processor, register)), 0], bytes: register.size() as u64 })
        }
        _ => {
            let operand = locate(processor, decoded);
            let length = u64::from(form.memory_bytes);
            if form.aligned && !operand.address.is_multiple_of(length) && !is_active(Bug::VexAlignedMisalignedNoFault) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            check_access(to_checked_flags(form, processor.flags), memory, operand, length, Access::Read)?;
            Ok(Source { bits: read_wide(memory, operand.address, length), bytes: length })
        }
    }
}

/// The flags as the form's access checks read them: AC left out for a form that never checks alignment.
fn to_checked_flags(form: &AvxForm, flags: Flags) -> Flags {
    if form.checks_alignment() { flags } else { flags.difference(Flags::ALIGNMENT_CHECK) }
}

/// The bytes of memory from the address, zero beyond `length`.
fn read_wide(memory: Memory<'_, '_>, address: u64, length: u64) -> Wide {
    let bytes: [u8; 2 * size_of::<u128>()] = read_bytes(memory, address);
    let mut wide = [u128::from_le_bytes(bytes[..16].try_into().expect("sixteen bytes")), u128::from_le_bytes(bytes[16..].try_into().expect("sixteen bytes"))];
    for (half, value) in wide.iter_mut().enumerate() {
        let kept = length.saturating_sub(16 * half as u64).min(16);
        *value &= if kept == 16 { u128::MAX } else { (1u128 << (8 * kept)) - 1 };
    }
    wide
}

/// Computes a floating-point operation, each half as the legacy arithmetic or [`crate::float`]'s VEX arithmetic
/// computes it, the halves' exceptions joined.
fn compute_float(
    form: &AvxForm,
    behavior: AvxBehavior,
    sources: &[Source],
    immediate: u8,
    mxcsr: u32,
    destination_bits: u32,
) -> Result<(Wide, u32), u32> {
    let first = sources.first().map_or([0; 2], |source| source.bits);
    let second = sources.get(1).map_or(first, |source| source.bits);
    let halves = if form.wide { 2 } else { 1 };
    let legacy = |operation, precision, arrangement| FloatForm {
        instruction: form.instruction,
        operation,
        precision,
        arrangement,
        place: form.place,
        code: form.code,
    };
    let to_wide = |bits: Vec<u128>| [bits[0], bits.get(1).copied().unwrap_or(0)];
    let results: Vec<Result<(u128, u32), u32>> = match form.operation {
        AvxOperation::Float { operation, precision, arrangement } => {
            let float = legacy(operation, precision, arrangement);
            let upper_from_second = is_active(Bug::VexScalarUpperFromSecond) && arrangement == Arrangement::Scalar;
            let kept = if upper_from_second { second } else { first };
            (0..halves).map(|half| float.apply(kept[half], second[half], mxcsr, immediate)).collect()
        }
        AvxOperation::Compare { precision, arrangement } => (0..halves)
            .map(|half| compare_half(first[half], second[half], precision, arrangement == Arrangement::Scalar, immediate, mxcsr))
            .collect(),
        AvxOperation::ConvertWidening(conversion) => {
            let precision = if conversion == Conversion::IntegerToFloat { Precision::Double } else { Precision::Single };
            let float = legacy(FloatOperation::Convert(conversion), precision, Arrangement::Packed);
            let shift = if is_active(Bug::WideningUpperFromLowLanes) { 0 } else { 64 };
            (0..halves).map(|half| float.apply(0, first[0] >> (shift * half), mxcsr, 0)).collect()
        }
        AvxOperation::ConvertNarrowing(conversion) => {
            let float = legacy(FloatOperation::Convert(conversion), Precision::Double, Arrangement::Packed);
            let narrowed: Vec<Result<(u128, u32), u32>> = (0..halves).map(|half| float.apply(0, first[half], mxcsr, 0)).collect();
            let (bits, flags) = join_halves(&narrowed, mxcsr)?;
            let joined = bits.iter().enumerate().fold(0, |all, (half, &value)| all | (value << (64 * half)));
            return Ok(([joined, 0], flags));
        }
        AvxOperation::IntegerToScalar(precision) => {
            let integer = match sources[1].bytes {
                4 => i64::from(second[0] as u32 as i32),
                _ if is_active(Bug::QuadwordToScalarLowHalf) => i64::from(second[0] as u32 as i32),
                _ => second[0] as u64 as i64,
            };
            vec_of(convert_integer(first[0], integer, precision, mxcsr))
        }
        AvxOperation::ScalarToInteger { precision, truncate } => {
            let (integer, flags) = convert_to_integer(first[0] as u64, precision, destination_bits, truncate, mxcsr)?;
            return Ok(([u128::from(integer), 0], flags));
        }
        AvxOperation::RoundToIntegral { precision, arrangement } => match arrangement {
            Arrangement::Scalar => vec_of(round_half(first[0], second[0], precision, true, immediate, mxcsr)),
            Arrangement::Packed => (0..halves).map(|half| round_half(0, first[half], precision, false, immediate, mxcsr)).collect(),
        },
        AvxOperation::Fused { sign, order, precision, arrangement } => {
            let third = sources[2].bits;
            let subtract = match sign {
                FusedSign::MultiplyAdd | FusedSign::NegatedMultiplyAdd => [false, false],
                FusedSign::MultiplySubtract | FusedSign::NegatedMultiplySubtract => [true, true],
                FusedSign::MultiplyAddSubtract => [true, false],
                FusedSign::MultiplySubtractAdd => [false, true],
            };
            let subtract = if is_active(Bug::FusedAlternationSwapped) && subtract[0] != subtract[1] {
                [subtract[1], subtract[0]]
            } else {
                subtract
            };
            let lanes = FusedLanes {
                roles: order.to_roles(),
                negate_product: matches!(sign, FusedSign::NegatedMultiplyAdd | FusedSign::NegatedMultiplySubtract),
                subtract,
                precision,
                scalar: arrangement == Arrangement::Scalar,
            };
            (0..halves).map(|half| fuse_half([first[half], second[half], third[half]], lanes, mxcsr, behavior.fused)).collect()
        }
        AvxOperation::HalfToSingle => (0..halves).map(|half| widen_halves(first[0] >> (64 * half), mxcsr)).collect(),
        AvxOperation::SingleToHalf => {
            let narrowed: Vec<Result<(u128, u32), u32>> = (0..halves).map(|half| narrow_to_halves(first[half], immediate, mxcsr)).collect();
            let (bits, flags) = join_halves(&narrowed, mxcsr)?;
            let joined = bits.iter().enumerate().fold(0, |all, (half, &value)| all | (value << (64 * half)));
            return Ok(([joined, 0], flags));
        }
        AvxOperation::StoreMxcsr => return Ok(([u128::from(mxcsr), 0], 0)),
        AvxOperation::DotProduct(precision) => {
            let low = dot_half(first[0], second[0], precision, immediate, mxcsr, 0)?;
            if !form.wide {
                return Ok(([low.0, 0], low.1));
            }
            let high = dot_half(first[1], second[1], precision, immediate, mxcsr, low.1)?;
            return Ok(([low.0, high.0], high.1));
        }
        operation => panic!("{operation:?} is computed elsewhere"),
    };
    let (bits, flags) = join_halves(&results, mxcsr)?;
    Ok((to_wide(bits), flags))
}

fn vec_of(result: Result<(u128, u32), u32>) -> Vec<Result<(u128, u32), u32>> {
    alloc::vec![result]
}

/// VMASKMOVPS to VPMASKMOVQ: each lane whose mask lane (the VEX.vvvv register) has its top bit set is loaded or
/// stored, and only those lanes' bytes are checked, lowest first; a load zeroes the others.
fn execute_masked(
    form: &AvxForm,
    behavior: AvxBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let lane = match form.operation {
        AvxOperation::MaskedLoad(lane) | AvxOperation::MaskedStore(lane) => lane,
        AvxOperation::MaskedByteStore => Lane::Byte,
        operation => panic!("{operation:?} is no masked form"),
    };
    let read_vector = |register: Register| {
        let number = register.number();
        [processor.vector_registers[number], processor.upper_vector_halves[number]]
    };
    let bytewise = form.operation == AvxOperation::MaskedByteStore;
    let (mask_register, value_register) = if bytewise { (decoded.op2_register(), decoded.op1_register()) } else { (decoded.op1_register(), decoded.op2_register()) };
    let mask = read_vector(mask_register);
    let bytes = u64::from(lane.to_bits() / u8::BITS);
    let halves = if form.wide { 2 } else { 1 };
    let per_half = (16 / bytes) as usize;
    let lane_mask = (1u128 << lane.to_bits()) - 1;
    let enabled: Vec<usize> = (0..halves * per_half)
        .filter(|&index| {
            let value = (mask[index / per_half] >> (lane.to_bits() as usize * (index % per_half))) & lane_mask;
            value >> (lane.to_bits() - 1) != 0
        })
        .collect();
    // VMASKMOVDQU stores at RDI, register 7.
    let operand = if bytewise { MemoryOperand { address: processor.general_purpose_registers[7], stack: false } } else { locate(processor, decoded) };
    let at = |index: usize| MemoryOperand { address: operand.address.wrapping_add(bytes * index as u64), stack: operand.stack };
    let access = if matches!(form.operation, AvxOperation::MaskedLoad(_)) { Access::Read } else { Access::Write };
    let check = |index: usize| -> Result<(), Exception> {
        check_canonical(at(index), bytes)?;
        match find_fault(memory, at(index).address, bytes, access) {
            None => Ok(()),
            Some((address, page_present)) => Err(Exception::PageFault { access, page_present, address }),
        }
    };
    // VMASKMOVDQU checks its high quadword and then its low one whatever its mask, as this chip does.
    let checked: Vec<usize> = if bytewise {
        (8..16).chain(0..8).collect()
    } else if is_active(Bug::MaskedAccessChecksEveryLane) {
        (0..halves * per_half).collect()
    } else {
        enabled.clone()
    };
    // VMASKMOVDQU's access is two quadwords, which AC checks for alignment whatever the mask selects.
    if bytewise {
        check_canonical(operand, 1)?;
        if processor.flags.contains(Flags::ALIGNMENT_CHECK) && !operand.address.is_multiple_of(8) {
            return Err(Exception::AlignmentCheck);
        }
    }
    let mut faulting = checked.iter().map(|&index| (index, check(index))).find(|(_, result)| result.is_err());
    // A store's page fault is reported at its first selected byte when that byte's page refuses it, else at its last
    // selected byte, as this chip reports it.
    if access == Access::Write && !bytewise && let Some((index, Err(Exception::PageFault { page_present, .. }))) = faulting {
        let (lowest, highest) = (checked[0], checked[checked.len() - 1]);
        let first = at(lowest).address;
        let address = if find_fault(memory, first, 1, access).is_some() { first } else { at(highest).address.wrapping_add(bytes - 1) };
        faulting = Some((index, Err(Exception::PageFault { access, page_present, address })));
    }
    let lane_value = |value: Wide, index: usize| (value[index / per_half] >> (lane.to_bits() as usize * (index % per_half))) & lane_mask;
    if matches!(form.operation, AvxOperation::MaskedLoad(_)) {
        if let Some((_, fault)) = faulting {
            return fault;
        }
        let mut result = [0u128; 2];
        for &index in &enabled {
            let value = read_wide(memory, at(index).address, bytes)[0];
            result[index / per_half] |= value << (lane.to_bits() as usize * (index % per_half));
        }
        let number = decoded.op0_register().number();
        processor.vector_registers[number] = result[0];
        processor.upper_vector_halves[number] = result[1];
        return Ok(());
    }
    let value = read_vector(value_register);
    let written: Vec<usize> = match faulting {
        Some((index, _)) if behavior.across_fault == StoreAcrossFault::WritesBytesBelowFault && bytewise => {
            enabled.iter().copied().filter(|&byte| byte / 8 != index / 8 && index < 8).collect()
        }
        Some((index, _)) if behavior.across_fault == StoreAcrossFault::WritesBytesBelowFault => {
            enabled.iter().copied().filter(|&below| below < index).collect()
        }
        Some(_) => Vec::new(),
        None => enabled.clone(),
    };
    for index in written {
        let bytes_of = lane_value(value, index).to_le_bytes();
        on_memory_mutated(MemoryDelta::ValueAssignment {
            start: at(index).address,
            bytes: alloc::borrow::Cow::Owned(bytes_of[..bytes as usize].to_vec()),
        });
    }
    match faulting {
        Some((_, fault)) => fault,
        None => Ok(()),
    }
}
